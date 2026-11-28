//! The GitHub App installation a team connected to its hub (ADR-0148 §8e): one each way, kept with who connected it.
//! The installation is checked its connector's before it gets here (`github_app::GitHubApp::installer_has`).
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::error::{Result, bad};
use crate::teams::Teams;

/// A team's connected installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connected {
    pub team_id: String,
    pub installation_id: i64,
    pub account: String,
    pub connected_by: String,
    pub connected_at: i64,
}

impl Connected {
    pub fn view(&self) -> Value {
        json!({ "installation": self.installation_id, "account": self.account, "connectedBy": self.connected_by, "connectedAt": self.connected_at })
    }
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Connected> {
    Ok(Connected { team_id: r.get(0)?, installation_id: r.get(1)?, account: r.get(2)?, connected_by: r.get(3)?, connected_at: r.get(4)? })
}

const COLS: &str = "team_id, installation_id, account, connected_by, connected_at";

impl Teams {
    /// Ties `installation` to `team_id` (in place of the one it had). One another team holds is refused: an
    /// installation is one team's.
    pub fn github_connect(&self, team_id: &str, installation: i64, account: &str, by: &str) -> Result<Connected> {
        let now = self.clock.now_ms();
        self.db.tx(|tx| {
            let other: Option<String> =
                tx.query_row("SELECT team_id FROM team_github WHERE installation_id = ?1", [installation], |r| r.get(0)).optional()?;
            if other.is_some_and(|t| t != team_id) {
                return Err(bad("That GitHub installation is another team's. Its owners can disconnect it first.", 409));
            }
            tx.execute(
                "INSERT INTO team_github (team_id, installation_id, account, connected_by, connected_at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(team_id) DO UPDATE SET installation_id = ?2, account = ?3, connected_by = ?4, connected_at = ?5",
                params![team_id, installation, account, by, now],
            )?;
            crate::rows::find::audit(tx, None, "github.connect", Some(&json!({ "team": team_id, "installation": installation })), now)?;
            Ok(())
        })?;
        Ok(Connected {
            team_id: team_id.into(),
            installation_id: installation,
            account: account.into(),
            connected_by: by.into(),
            connected_at: now,
        })
    }

    /// `team_id`'s installation, if it connected one.
    pub fn github_of(&self, team_id: &str) -> Result<Option<Connected>> {
        self.db.with(|c| c.query_row(&format!("SELECT {COLS} FROM team_github WHERE team_id = ?1"), [team_id], row).optional())
    }

    /// The team an installation is connected to.
    pub fn github_by_installation(&self, installation: i64) -> Result<Option<Connected>> {
        self.db.with(|c| c.query_row(&format!("SELECT {COLS} FROM team_github WHERE installation_id = ?1"), [installation], row).optional())
    }

    /// Unties `team_id`'s installation: the one it was, if any (the App stays installed on GitHub until its owner removes it).
    pub fn github_disconnect(&self, team_id: &str) -> Result<Option<i64>> {
        let had = self.github_of(team_id)?;
        if let Some(c) = &had {
            let now = self.clock.now_ms();
            self.db.tx(|tx| {
                tx.execute("DELETE FROM team_github WHERE team_id = ?1", [team_id])?;
                crate::rows::find::audit(
                    tx,
                    None,
                    "github.disconnect",
                    Some(&json!({ "team": team_id, "installation": c.installation_id })),
                    now,
                )?;
                Ok(())
            })?;
        }
        Ok(had.map(|c| c.installation_id))
    }

    /// GitHub removed the App from an account (`installation.deleted`): whichever team had it, untied.
    pub fn github_gone(&self, installation: i64) -> Result<Option<String>> {
        let Some(c) = self.github_by_installation(installation)? else { return Ok(None) };
        self.github_disconnect(&c.team_id)?;
        Ok(Some(c.team_id))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::accounts::Accounts;
    use crate::clock::ManualClock;
    use crate::db::Db;

    #[test]
    fn an_installation_is_one_teams_and_goes_with_a_disconnect_or_an_uninstall() {
        let db = Arc::new(Db::memory().unwrap());
        let clock = ManualClock::new(1_000);
        let a = Accounts { db: db.clone(), clock: clock.clone(), telemetry: None };
        let t = Teams { db: db.clone(), clock };
        let ada = a.dev_user("ada").unwrap();
        t.create(&ada, "acme", "").unwrap();
        t.create(&ada, "zeta", "").unwrap();
        let (acme, zeta) = (t.hub("acme").unwrap().unwrap(), t.hub("zeta").unwrap().unwrap());
        let c = t.github_connect(&acme.team_id, 7, "acme-org", "ada").unwrap();
        assert_eq!((c.installation_id, c.account.as_str()), (7, "acme-org"));
        assert_eq!(t.github_by_installation(7).unwrap().unwrap().team_id, acme.team_id);
        let e = t.github_connect(&zeta.team_id, 7, "acme-org", "ada").unwrap_err();
        assert!(e.to_string().contains("another team's"), "{e}");
        // connecting again replaces the team's own
        t.github_connect(&acme.team_id, 8, "acme-two", "ada").unwrap();
        assert!(t.github_by_installation(7).unwrap().is_none());
        assert_eq!(t.github_disconnect(&acme.team_id).unwrap(), Some(8));
        assert!(t.github_of(&acme.team_id).unwrap().is_none());
        t.github_connect(&zeta.team_id, 9, "z", "ada").unwrap();
        assert_eq!(t.github_gone(9).unwrap(), Some(zeta.team_id.clone()));
        assert_eq!(t.github_gone(9).unwrap(), None);
    }
}
