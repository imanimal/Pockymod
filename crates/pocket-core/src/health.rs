//! The Health panel: missing frameworks, version trouble after game patches, conflicts, files changed outside
//! the app, and REDmod state. Each module turns red when something is broken; the report names one fix to do first.

use crate::classify::ModKind;
use crate::frameworks::{self, Framework, InstalledFramework};
use crate::game::GameInstall;
use crate::library::InstalledMod;
use crate::redmod::DeployOutcome;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ok,
    Warn,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FixAction {
    SetGameFolder,
    #[serde(rename_all = "camelCase")]
    InstallFramework { framework: Framework, mod_id: u64 },
    #[serde(rename_all = "camelCase")]
    UpdateFramework { framework: Framework, mod_id: u64 },
    RunRedmodDeploy,
    InstallRedmodDlc,
    #[serde(rename_all = "camelCase")]
    Redeploy { key: String },
    CheckUpdates,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub action: FixAction,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthItem {
    pub title: String,
    pub detail: String,
    pub status: Status,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthModule {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub items: Vec<HealthItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub modules: Vec<HealthModule>,
    pub worst: Status,
    /// The one orange key: the most important fix.
    pub primary_fix: Option<Fix>,
    pub problems: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictRecord {
    pub rel: String,
    pub winner: String,
    pub loser: String,
}

pub struct HealthInput<'a> {
    pub game: Option<&'a GameInstall>,
    pub frameworks: &'a [InstalledFramework],
    /// Mods enabled in the active profile.
    pub mods: &'a [InstalledMod],
    pub conflicts: &'a [ConflictRecord],
    /// (mod key, mod name, files gone or changed)
    pub drift: &'a [(String, String, usize)],
    pub redmods_present: usize,
    pub redmod_last: Option<&'a DeployOutcome>,
    /// REDmods or their order changed since the last successful deploy.
    pub redmod_dirty: bool,
}

fn module(id: &str, title: &str, items: Vec<HealthItem>) -> HealthModule {
    let status = items.iter().map(|i| i.status).max().unwrap_or(Status::Ok);
    HealthModule { id: id.into(), title: title.into(), status, items }
}

fn item(title: impl Into<String>, detail: impl Into<String>, status: Status, fix: Option<Fix>) -> HealthItem {
    HealthItem { title: title.into(), detail: detail.into(), status, fix }
}

pub fn check(input: &HealthInput) -> HealthReport {
    let mut modules = Vec::new();

    // game -----------------------------------------------------------------------------------------------
    let mut g = Vec::new();
    match input.game {
        None => g.push(item(
            "Game not found",
            "Pocket Mods couldn't find Cyberpunk 2077. Pick its folder (the one with bin\\x64\\Cyberpunk2077.exe).",
            Status::Bad,
            Some(Fix { action: FixAction::SetGameFolder, label: "Pick the game folder".into() }),
        )),
        Some(game) => {
            g.push(item(
                format!("Cyberpunk 2077 {}", game.version.clone().unwrap_or_else(|| "(version unknown)".into())),
                game.root.display().to_string(),
                Status::Ok,
                None,
            ));
            if game.under_program_files {
                g.push(item(
                    "Game is under Program Files",
                    "Windows may block writes there. If installs fail with a permission error, move the game to another library folder.",
                    Status::Warn,
                    None,
                ));
            }
        }
    }
    modules.push(module("game", "Game", g));

    // frameworks -----------------------------------------------------------------------------------------
    let build_time = input.game.and_then(|g| g.build_time);
    let wanted: Vec<Framework> = {
        let mut w: Vec<Framework> = input.mods.iter().flat_map(|m| m.frameworks.iter().copied()).collect();
        w.sort();
        w.dedup();
        frameworks::closure(&w)
    };
    let mut f = Vec::new();
    for fw in frameworks::FRAMEWORKS {
        let inst = input.frameworks.iter().find(|i| i.id == fw.id);
        let installed = inst.is_some_and(|i| i.installed);
        let needed_by = input.mods.iter().filter(|m| m.frameworks.contains(&fw.id)).count();
        let needed = wanted.contains(&fw.id);
        if needed && !installed {
            f.push(item(
                format!("{} is missing", fw.name),
                if needed_by > 0 { format!("{needed_by} enabled mod(s) need it.") } else { "Another framework you use needs it.".into() },
                Status::Bad,
                Some(Fix { action: FixAction::InstallFramework { framework: fw.id, mod_id: fw.nexus_mod_id }, label: format!("Install {}", fw.name) }),
            ));
        } else if installed {
            let inst = inst.unwrap();
            let stale = matches!((inst.file_time, build_time), (Some(ft), Some(bt)) if ft < bt);
            let version = inst.version.clone().map(|v| format!(" {v}")).unwrap_or_default();
            if stale {
                f.push(item(
                    format!("{}{version} may be outdated", fw.name),
                    "It was installed before the game's last update. Frameworks often break on game patches; check for a newer version before playing.",
                    if needed { Status::Bad } else { Status::Warn },
                    Some(Fix { action: FixAction::UpdateFramework { framework: fw.id, mod_id: fw.nexus_mod_id }, label: format!("Update {}", fw.name) }),
                ));
            } else {
                f.push(item(format!("{}{version}", fw.name), if needed_by > 0 { format!("Installed. Used by {needed_by} mod(s).") } else { "Installed.".into() }, Status::Ok, None));
            }
        }
    }
    modules.push(module("frameworks", "Frameworks", f));

    // patch risk -----------------------------------------------------------------------------------------
    let mut p = Vec::new();
    if let Some(bt) = build_time {
        let risky: Vec<&InstalledMod> = input
            .mods
            .iter()
            .filter(|m| matches!(m.kind, ModKind::Script | ModKind::Redmod))
            .filter(|m| m.uploaded_at.is_some_and(|u| u < bt))
            .filter(|m| m.update_available.is_none())
            .collect();
        if !risky.is_empty() {
            p.push(item(
                format!("{} script mod(s) predate this game build", risky.len()),
                risky.iter().take(6).map(|m| m.name.as_str()).collect::<Vec<_>>().join(", "),
                Status::Warn,
                Some(Fix { action: FixAction::CheckUpdates, label: "Check for updates".into() }),
            ));
        }
    }
    let updates: Vec<&InstalledMod> = input.mods.iter().filter(|m| m.update_available.is_some()).collect();
    if !updates.is_empty() {
        p.push(item(
            format!("{} update(s) available", updates.len()),
            updates.iter().take(6).map(|m| m.name.as_str()).collect::<Vec<_>>().join(", "),
            Status::Warn,
            None,
        ));
    }
    modules.push(module("versions", "Versions", p));

    // conflicts ------------------------------------------------------------------------------------------
    let c: Vec<HealthItem> = input
        .conflicts
        .iter()
        .map(|c| item(format!("{} and {} both write {}", c.winner, c.loser, c.rel.rsplit('/').next().unwrap_or(&c.rel)), format!("{} wins ({}).", c.winner, c.rel), Status::Warn, None))
        .collect();
    modules.push(module("conflicts", "Conflicts", c));

    // files changed outside -------------------------------------------------------------------------------
    let d: Vec<HealthItem> = input
        .drift
        .iter()
        .map(|(key, name, n)| {
            item(
                format!("{name}: {n} file(s) missing or changed"),
                "Something outside Pocket Mods removed or replaced them (a game update, file verification, or another manager).",
                Status::Warn,
                Some(Fix { action: FixAction::Redeploy { key: key.clone() }, label: format!("Redeploy {name}") }),
            )
        })
        .collect();
    modules.push(module("files", "Files", d));

    // REDmod ---------------------------------------------------------------------------------------------
    let mut r = Vec::new();
    if input.redmods_present > 0 {
        let tool = input.game.is_some_and(|g| g.redmod_tool);
        if !tool {
            r.push(item(
                "REDmod tool is not installed",
                format!("{} REDmod(s) won't load until the free REDmod DLC is installed (Steam: game Properties, DLC; GOG Galaxy: DLC).", input.redmods_present),
                Status::Bad,
                Some(Fix { action: FixAction::InstallRedmodDlc, label: "How to install REDmod".into() }),
            ));
        } else if let Some(last) = input.redmod_last.filter(|l| !l.ok) {
            r.push(item(
                "Last REDmod deploy failed",
                last.problems.first().cloned().unwrap_or_else(|| "See the log.".into()),
                Status::Bad,
                Some(Fix { action: FixAction::RunRedmodDeploy, label: "Deploy REDmods again".into() }),
            ));
        } else if input.redmod_dirty || input.redmod_last.is_none() {
            r.push(item(
                "REDmods need deploying",
                "REDmods or their load order changed since the last deploy.",
                Status::Warn,
                Some(Fix { action: FixAction::RunRedmodDeploy, label: "Deploy REDmods".into() }),
            ));
        } else {
            r.push(item(format!("{} REDmod(s) deployed", input.redmods_present), "", Status::Ok, None));
        }
    }
    modules.push(module("redmod", "REDmod", r));

    let worst = modules.iter().map(|m| m.status).max().unwrap_or(Status::Ok);
    let problems = modules.iter().flat_map(|m| &m.items).filter(|i| i.status != Status::Ok).count();
    let primary_fix = [Status::Bad, Status::Warn]
        .iter()
        .find_map(|s| modules.iter().flat_map(|m| &m.items).filter(|i| i.status == *s).find_map(|i| i.fix.clone()));
    HealthReport { modules, worst, primary_fix, problems }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Launcher;
    use crate::library::sample_mod;

    fn game(build: i64) -> GameInstall {
        GameInstall { root: "G:\\CP".into(), launcher: Launcher::Steam, version: Some("2.3".into()), build_time: Some(build), under_program_files: false, redmod_tool: true }
    }

    fn input<'a>(g: Option<&'a GameInstall>, fw: &'a [InstalledFramework], mods: &'a [InstalledMod]) -> HealthInput<'a> {
        HealthInput { game: g, frameworks: fw, mods, conflicts: &[], drift: &[], redmods_present: 0, redmod_last: None, redmod_dirty: false }
    }

    #[test]
    fn no_game_is_the_first_fix() {
        let r = check(&input(None, &[], &[]));
        assert_eq!(r.worst, Status::Bad);
        assert_eq!(r.primary_fix.unwrap().action, FixAction::SetGameFolder);
    }

    #[test]
    fn missing_framework_and_its_dependencies() {
        let g = game(1000);
        let mut m = sample_mod("1-1", "Outfit");
        m.frameworks = vec![Framework::ArchiveXl];
        let mods = [m];
        let r = check(&input(Some(&g), &[], &mods));
        let fw = r.modules.iter().find(|m| m.id == "frameworks").unwrap();
        assert_eq!(fw.status, Status::Bad);
        let titles: Vec<&str> = fw.items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"ArchiveXL is missing") && titles.contains(&"RED4ext is missing"), "{titles:?}");
        // RED4ext comes first in the table, so it's the first fix
        assert!(matches!(r.primary_fix.unwrap().action, FixAction::InstallFramework { framework: Framework::Red4ext, .. }));
    }

    #[test]
    fn stale_framework_after_patch() {
        let g = game(1000);
        let fw = [InstalledFramework { id: Framework::Cet, name: "Cyber Engine Tweaks".into(), installed: true, version: Some("1.30".into()), file_time: Some(500) }];
        let r = check(&input(Some(&g), &fw, &[]));
        let f = r.modules.iter().find(|m| m.id == "frameworks").unwrap();
        assert_eq!(f.status, Status::Warn, "not needed by anything, so a warning");
        assert!(f.items[0].title.contains("may be outdated"));
        let fresh = [InstalledFramework { file_time: Some(2000), ..fw[0].clone() }];
        assert_eq!(check(&input(Some(&g), &fresh, &[])).worst, Status::Ok);
    }

    #[test]
    fn script_mods_older_than_the_build_are_flagged() {
        let g = game(1000);
        let mut m = sample_mod("1-1", "Old script");
        m.kind = ModKind::Script;
        m.uploaded_at = Some(10);
        let mods = [m];
        let r = check(&input(Some(&g), &[], &mods));
        assert_eq!(r.modules.iter().find(|m| m.id == "versions").unwrap().status, Status::Warn);
    }

    #[test]
    fn redmod_states() {
        let g = game(1000);
        let mut i = input(Some(&g), &[], &[]);
        i.redmods_present = 2;
        i.redmod_dirty = true;
        let r = check(&i);
        assert_eq!(r.primary_fix.unwrap().action, FixAction::RunRedmodDeploy);
        let failed = DeployOutcome { ok: false, problems: vec!["boom".into()], log_tail: vec![], at: 0 };
        i.redmod_last = Some(&failed);
        assert_eq!(check(&i).modules.iter().find(|m| m.id == "redmod").unwrap().status, Status::Bad);
        let no_tool = GameInstall { redmod_tool: false, ..g.clone() };
        i.game = Some(&no_tool);
        assert_eq!(check(&i).primary_fix.unwrap().action, FixAction::InstallRedmodDlc);
    }
}
