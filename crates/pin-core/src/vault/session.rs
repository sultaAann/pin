use std::time::{Duration, Instant, SystemTime};

use rusqlite::{Connection, OptionalExtension, params};

use super::crypto::{self, SecretKey};
use super::{KdfParams, MasterPassword, RecoveryKey, Vault, VaultStatus};
use crate::error::{Error, Result};
use crate::random;
use crate::store::write_tx;

pub(super) const UID: usize = 16;
const SALT_LEN: usize = 16;

const AAD_PASSWORD_WRAP: &[u8] = b"pin/vault/password-wrap";
const AAD_RECOVERY_WRAP: &[u8] = b"pin/vault/recovery-wrap";

pub(crate) struct Session {
    key: SecretKey,
    uid: [u8; UID],
    last_used: Instant,
    last_used_wall: SystemTime,
}

impl Session {
    fn new(key: SecretKey, uid: [u8; UID]) -> Self {
        Self {
            key,
            uid,
            last_used: Instant::now(),
            last_used_wall: SystemTime::now(),
        }
    }

    // Instant stops during system sleep on macOS and Linux, so wall-clock time is checked too;
    // a wall clock moved backwards counts as idle.
    fn is_idle_at(&self, timeout: Option<Duration>, now: Instant, wall: SystemTime) -> bool {
        timeout.is_some_and(|t| {
            now.saturating_duration_since(self.last_used) >= t
                || wall
                    .duration_since(self.last_used_wall)
                    .map_or(true, |e| e >= t)
        })
    }

    fn is_idle(&self, timeout: Option<Duration>) -> bool {
        self.is_idle_at(timeout, Instant::now(), SystemTime::now())
    }

    fn deadline(&self, timeout: Duration) -> Instant {
        let now = Instant::now();
        let idle = now.saturating_duration_since(self.last_used).max(
            SystemTime::now()
                .duration_since(self.last_used_wall)
                .unwrap_or(timeout),
        );
        now + timeout.saturating_sub(idle)
    }

    fn touch(&mut self) {
        self.last_used = Instant::now();
        self.last_used_wall = SystemTime::now();
    }

    pub(super) fn seal(&self, label: &[u8], aad: &[&[u8]], plaintext: &[u8]) -> Result<Vec<u8>> {
        crypto::seal(&self.key, &self.aad(label, aad), plaintext)
    }

    pub(super) fn open(
        &self,
        label: &[u8],
        aad: &[&[u8]],
        sealed: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        crypto::open(&self.key, &self.aad(label, aad), sealed)
    }

    fn aad<'b>(&'b self, label: &'b [u8], aad: &[&'b [u8]]) -> Vec<&'b [u8]> {
        let mut parts = Vec::with_capacity(aad.len() + 2);
        parts.push(label);
        parts.push(&self.uid[..]);
        parts.extend_from_slice(aad);
        parts
    }
}

struct VaultRow {
    uid: [u8; UID],
    salt: [u8; SALT_LEN],
    kdf: KdfParams,
    password_wrap: Vec<u8>,
    recovery_wrap: Vec<u8>,
}

struct Wraps {
    salt: [u8; SALT_LEN],
    kdf: KdfParams,
    password: Vec<u8>,
}

impl Vault<'_> {
    pub fn status(&mut self) -> Result<VaultStatus> {
        if is_valid(self.session, self.conn, self.options.auto_lock)? {
            return Ok(VaultStatus::Unlocked);
        }
        Ok(if load_vault(self.conn)?.is_some() {
            VaultStatus::Locked
        } else {
            VaultStatus::Missing
        })
    }

    pub fn create(&mut self, password: &MasterPassword) -> Result<RecoveryKey> {
        if load_vault(self.conn)?.is_some() {
            return Err(Error::VaultExists);
        }
        let uid: [u8; UID] = random::array()?;
        let key = SecretKey::random()?;
        let recovery = RecoveryKey::generate()?;
        let wraps = wrap_with_password(&key, &uid, password, self.options.kdf)?;
        let recovery_wrap = wrap_with_recovery(&key, &uid, &recovery)?;

        let tx = write_tx(self.conn)?;
        if load_vault(&tx)?.is_some() {
            return Err(Error::VaultExists);
        }
        tx.execute(
            "INSERT INTO vault (id, uid, salt, kdf_memory_kib, kdf_iterations, kdf_parallelism,
                                password_wrap, recovery_wrap)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &uid[..],
                &wraps.salt[..],
                wraps.kdf.memory_kib,
                wraps.kdf.iterations,
                wraps.kdf.parallelism,
                wraps.password,
                recovery_wrap,
            ],
        )?;
        tx.commit()?;
        *self.session = Some(Session::new(key, uid));
        Ok(recovery)
    }

    pub fn unlock(&mut self, password: &MasterPassword) -> Result<()> {
        let row = load_vault(self.conn)?.ok_or(Error::VaultMissing)?;
        let key = unwrap_with_password(&row, password)?;
        *self.session = Some(Session::new(key, row.uid));
        Ok(())
    }

    pub fn recover(
        &mut self,
        recovery: &RecoveryKey,
        new_password: &MasterPassword,
    ) -> Result<RecoveryKey> {
        let row = load_vault(self.conn)?.ok_or(Error::VaultMissing)?;
        let key = unwrap_key(
            &recovery.wrapping_key(),
            &[AAD_RECOVERY_WRAP, &row.uid],
            &row.recovery_wrap,
            Error::InvalidRecoveryKey,
        )?;
        let fresh = RecoveryKey::generate()?;
        let wraps = wrap_with_password(&key, &row.uid, new_password, self.options.kdf)?;
        let recovery_wrap = wrap_with_recovery(&key, &row.uid, &fresh)?;
        self.replace_wraps(&row, &wraps, Some(&recovery_wrap))?;
        *self.session = Some(Session::new(key, row.uid));
        Ok(fresh)
    }

    pub fn change_password(&mut self, old: &MasterPassword, new: &MasterPassword) -> Result<()> {
        let row = load_vault(self.conn)?.ok_or(Error::VaultMissing)?;
        let key = unwrap_with_password(&row, old)?;
        let wraps = wrap_with_password(&key, &row.uid, new, self.options.kdf)?;
        self.replace_wraps(&row, &wraps, None)?;
        *self.session = Some(Session::new(key, row.uid));
        Ok(())
    }

    pub fn lock(&mut self) {
        *self.session = None;
    }

    pub fn lock_if_idle(&mut self) -> bool {
        let idle = self
            .session
            .as_ref()
            .is_some_and(|s| s.is_idle(self.options.auto_lock));
        if idle {
            *self.session = None;
        }
        idle
    }

    // Lets the app schedule one wake-up for auto-lock instead of polling while it is idle. Instant
    // stops during system sleep, so the app must also call lock_if_idle on resume.
    pub fn lock_deadline(&self) -> Option<Instant> {
        let timeout = self.options.auto_lock?;
        self.session.as_ref().map(|s| s.deadline(timeout))
    }

    pub fn reset(&mut self) -> Result<()> {
        *self.session = None;
        let tx = write_tx(self.conn)?;
        tx.execute("DELETE FROM secrets", [])?;
        tx.execute("DELETE FROM vault", [])?;
        tx.commit()?;
        Ok(())
    }

    // The KDF runs before the transaction; the compare-and-swap on the old wrap rejects a
    // concurrent password change or recovery instead of silently overwriting it.
    fn replace_wraps(
        &mut self,
        row: &VaultRow,
        wraps: &Wraps,
        recovery_wrap: Option<&[u8]>,
    ) -> Result<()> {
        let tx = write_tx(self.conn)?;
        let changed = tx.execute(
            "UPDATE vault SET salt = ?3, kdf_memory_kib = ?4, kdf_iterations = ?5,
                              kdf_parallelism = ?6, password_wrap = ?7,
                              recovery_wrap = COALESCE(?8, recovery_wrap)
             WHERE id = 1 AND uid = ?1 AND password_wrap = ?2 AND recovery_wrap = ?9",
            params![
                &row.uid[..],
                row.password_wrap,
                &wraps.salt[..],
                wraps.kdf.memory_kib,
                wraps.kdf.iterations,
                wraps.kdf.parallelism,
                wraps.password,
                recovery_wrap,
                row.recovery_wrap,
            ],
        )?;
        if changed == 0 {
            return Err(if load_vault(&tx)?.is_some() {
                Error::VaultChanged
            } else {
                Error::VaultMissing
            });
        }
        tx.commit()?;
        Ok(())
    }
}

// Every secret operation goes through here so an idle or stale (reset by another process) vault
// key is dropped before it can read or write anything.
pub(super) fn session<'s>(
    slot: &'s mut Option<Session>,
    conn: &Connection,
    auto_lock: Option<Duration>,
) -> Result<&'s Session> {
    if !is_valid(slot, conn, auto_lock)? {
        return Err(Error::VaultLocked);
    }
    let current = slot.as_mut().ok_or(Error::VaultLocked)?;
    current.touch();
    Ok(current)
}

fn is_valid(
    slot: &mut Option<Session>,
    conn: &Connection,
    auto_lock: Option<Duration>,
) -> Result<bool> {
    let Some(current) = slot.as_ref() else {
        return Ok(false);
    };
    if current.is_idle(auto_lock) {
        *slot = None;
        return Ok(false);
    }
    let uid: Option<Vec<u8>> = conn
        .query_row("SELECT uid FROM vault WHERE id = 1", [], |row| row.get(0))
        .optional()?;
    if uid.as_deref() != Some(&current.uid[..]) {
        *slot = None;
        return Ok(false);
    }
    Ok(true)
}

fn wrap_with_password(
    key: &SecretKey,
    uid: &[u8; UID],
    password: &MasterPassword,
    kdf: KdfParams,
) -> Result<Wraps> {
    let salt: [u8; SALT_LEN] = random::array()?;
    let kek = crypto::derive(password.as_bytes(), &salt, kdf)?;
    let password = crypto::seal(&kek, &[AAD_PASSWORD_WRAP, uid], key.as_bytes())?;
    Ok(Wraps {
        salt,
        kdf,
        password,
    })
}

fn wrap_with_recovery(key: &SecretKey, uid: &[u8; UID], recovery: &RecoveryKey) -> Result<Vec<u8>> {
    crypto::seal(
        &recovery.wrapping_key(),
        &[AAD_RECOVERY_WRAP, uid],
        key.as_bytes(),
    )
}

fn unwrap_with_password(row: &VaultRow, password: &MasterPassword) -> Result<SecretKey> {
    let kek = crypto::derive(password.as_bytes(), &row.salt, row.kdf)?;
    unwrap_key(
        &kek,
        &[AAD_PASSWORD_WRAP, &row.uid],
        &row.password_wrap,
        Error::WrongPassword,
    )
}

fn unwrap_key(
    kek: &SecretKey,
    aad: &[&[u8]],
    wrapped: &[u8],
    on_auth_failure: Error,
) -> Result<SecretKey> {
    let plain = crypto::open(kek, aad, wrapped).map_err(|_| on_auth_failure)?;
    SecretKey::from_slice(&plain)
}

fn load_vault(conn: &Connection) -> Result<Option<VaultRow>> {
    let row = conn
        .query_row(
            "SELECT uid, salt, kdf_memory_kib, kdf_iterations, kdf_parallelism,
                    password_wrap, recovery_wrap
             FROM vault WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, Vec<u8>>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((uid, salt, memory, iterations, parallelism, password_wrap, recovery_wrap)) = row
    else {
        return Ok(None);
    };
    let param = |v: i64| u32::try_from(v).map_err(|_| Error::Corrupt);
    Ok(Some(VaultRow {
        uid: uid.try_into().map_err(|_| Error::Corrupt)?,
        salt: salt.try_into().map_err(|_| Error::Corrupt)?,
        kdf: KdfParams {
            memory_kib: param(memory)?,
            iterations: param(iterations)?,
            parallelism: param(parallelism)?,
        },
        password_wrap,
        recovery_wrap,
    }))
}

pub(super) fn uid_from(bytes: &[u8]) -> Result<[u8; UID]> {
    bytes.try_into().map_err(|_| Error::Corrupt)
}

pub(super) fn new_uid() -> Result<[u8; UID]> {
    random::array()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn session() -> Session {
        Session::new(SecretKey::random().unwrap(), [0; UID])
    }

    #[test]
    fn idle_after_timeout_on_either_clock() {
        let s = session();
        let t = Some(Duration::from_mins(1));
        let (now, wall) = (s.last_used, s.last_used_wall);
        assert!(!s.is_idle_at(
            t,
            now + Duration::from_secs(59),
            wall + Duration::from_secs(59)
        ));
        assert!(s.is_idle_at(t, now + Duration::from_mins(1), wall));
        assert!(s.is_idle_at(t, now, wall + Duration::from_hours(1)));
        assert!(!s.is_idle_at(None, now + Duration::from_hours(1), wall));
    }

    #[test]
    fn deadline_accounts_for_wall_clock_idle_time() {
        let mut s = session();
        let timeout = Duration::from_mins(5);
        let before = Instant::now();
        assert!(s.deadline(timeout) + Duration::from_secs(1) >= before + timeout);
        s.last_used_wall -= Duration::from_mins(4);
        assert!(s.deadline(timeout) <= Instant::now() + Duration::from_mins(1));
        s.last_used_wall -= Duration::from_mins(10);
        assert!(s.deadline(timeout) <= Instant::now());
    }

    #[test]
    fn wall_clock_moved_backwards_counts_as_idle() {
        let s = session();
        let earlier = s.last_used_wall - Duration::from_secs(1);
        assert!(s.is_idle_at(Some(Duration::from_mins(1)), s.last_used, earlier));
    }
}
