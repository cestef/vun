//! What a command's path names (decision 356), and workspaces (spec 12, section 5): a root
//! package listing members below it, which are one project and share its lockfile.

use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_build::lock::{self, Lock};
use vun_diag::{DiagId, errors};
use vun_driver::Session;
use vun_driver::subject::Subject;

use crate::common::Fixture;

fn reported(session: &Session) -> Vec<DiagId> {
    session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect()
}

#[test]
fn a_path_names_a_loose_file_a_package_or_a_part_of_one() {
    let fx = Fixture::new("subject");
    let app = Fixture::manifest("app", 1, "0.1.0");
    let dir = fx.write("app", &app, "x := 1;\n");
    let file = Fixture::file(&dir, &app);
    let loose = fx.root.write("notes.vun", "y := 2;\n");
    let mut session = fx.session();
    let named = |s: &mut Session, path: &Path| s.subject(Some(path)).unwrap();

    assert_eq!(named(&mut session, &loose), Some(Subject::Loose(loose.clone())));
    let package = Subject::Package { dir: dir.clone(), within: None };
    assert_eq!(named(&mut session, &dir), Some(package.clone()));
    assert_eq!(named(&mut session, &dir.join(MANIFEST)), Some(package.clone()), "its manifest names it too");
    let part = named(&mut session, &file).unwrap();
    assert_eq!(part, Subject::Package { dir: dir.clone(), within: Some(file.clone()) });
    assert_eq!((part.file(), package.file()), (Some(file.as_path()), None));
    assert_eq!(session.files(&package), [dir.join(MANIFEST), file.clone()], "the manifest first");
    assert_eq!(session.files(&part), [file]);
    assert!(!session.failed(), "{:?}", reported(&session));

    // A directory outside any package names nothing, and says so.
    let outside = fx.root.join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    assert_eq!(named(&mut session, &outside), None);
    assert_eq!(reported(&session), [errors::PACKAGE_NO_PROJECT]);
}

#[test]
fn a_workspace_is_its_root_and_members_which_share_its_lockfile() {
    let fx = Fixture::new("workspace");
    let hal = Fixture::manifest("hal", 7, "1.0.0");
    fx.write("hal", &hal, "a := 1;\n");
    let (url, hash) = fx.publish("hal", &hal);
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.deps.push(Fixture::stored("hal", "1.0.0", &url, hash));
    let mut tool = Fixture::manifest("tool", 2, "0.1.0");
    tool.deps.push(Fixture::local("app", "../app"));
    let mut root = Fixture::manifest("ws", 3, "0.1.0");
    root.members = vec!["pkgs/app".into(), "pkgs/tool".into()];
    let ws = fx.write("ws", &root, "");
    let app_dir = fx.write("ws/pkgs/app", &app, "x := 1;\n");
    let tool_dir = fx.write("ws/pkgs/tool", &tool, "y := 2;\n");

    let mut session = fx.session();
    let workspace = session.subject(Some(&ws)).unwrap().unwrap();
    let members: Vec<PathBuf> = ["pkgs/app", "pkgs/tool"].iter().map(|m| ws.join(m)).collect();
    assert_eq!(workspace, Subject::Workspace { dir: ws.clone(), members: members.clone() });
    assert_eq!(workspace.packages(), [ws.clone(), members[0].clone(), members[1].clone()]);
    assert_eq!(session.files(&workspace).len(), 6, "each package's manifest and root file");
    let member = Subject::Package { dir: app_dir.clone(), within: None };
    assert_eq!(session.subject(Some(&app_dir)).unwrap(), Some(member), "a member is a package of its own");

    // Readying a member's file fetches for the whole workspace and writes the root's lockfile.
    session.prepare(&Fixture::file(&tool_dir, &tool));
    assert!(!session.failed(), "{:?}", reported(&session));
    assert!(!app_dir.join(lock::FILE).exists() && !tool_dir.join(lock::FILE).exists());
    let recorded = Lock::parse(&std::fs::read_to_string(ws.join(lock::FILE)).unwrap()).unwrap();
    let entries: Vec<_> = recorded.entries().iter().map(|e| (e.name.as_str(), e.via.as_deref())).collect();
    assert_eq!(entries, [("hal", Some("app"))], "the other member's dependency is recorded");
}

#[test]
fn a_member_holding_no_package_is_reported() {
    let fx = Fixture::new("workspace-missing");
    let mut root = Fixture::manifest("ws", 3, "0.1.0");
    root.members = vec!["pkgs/gone".into()];
    let ws = fx.write("ws", &root, "");
    let mut session = fx.session();
    assert_eq!(session.subject(Some(&ws)).unwrap(), None);
    assert_eq!(reported(&session), [errors::PACKAGE_MISSING_MEMBER]);
}

#[test]
fn run_takes_the_one_program_built() {
    let fx = Fixture::new("to-run");
    let mut session = fx.session();
    let program = PathBuf::from("app");
    assert_eq!(session.to_run(vec![program.clone()]), Some(program.clone()));
    assert_eq!(session.to_run(Vec::new()), None);
    assert_eq!(session.to_run(vec![program.clone(), PathBuf::from("tool")]), None);
    assert_eq!(reported(&session), [errors::PLAN_AMBIGUOUS_RUN, errors::PLAN_NOTHING_TO_RUN], "sorted by id");
}
