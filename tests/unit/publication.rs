use super::publish;
use crate::Error;

#[test]
fn partial_publication_restores_both_original_databases() {
    std::fs::create_dir_all(".tmp").unwrap();
    let directory = tempfile::tempdir_in(".tmp").unwrap();
    let staging = directory.path().join("stage");
    let destination = directory.path().join("dest");
    std::fs::create_dir(&staging).unwrap();
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("m.db"), b"old main").unwrap();
    std::fs::write(destination.join("hm.db"), b"old history").unwrap();
    std::fs::write(staging.join("m.db"), b"new main").unwrap();
    let error = publish(&staging, &destination).unwrap_err();
    let Error::Publication {
        rollback, recovery, ..
    } = error
    else {
        panic!("wrong error")
    };
    assert!(rollback.is_empty());
    assert!(recovery.is_none());
    assert_eq!(
        std::fs::read(destination.join("m.db")).unwrap(),
        b"old main"
    );
    assert_eq!(
        std::fs::read(destination.join("hm.db")).unwrap(),
        b"old history"
    );
}

#[test]
fn rollback_failure_keeps_recoverable_original_database_files() {
    std::fs::create_dir_all(".tmp").unwrap();
    let directory = tempfile::tempdir_in(".tmp").unwrap();
    let staging = directory.path().join("stage");
    let destination = directory.path().join("dest");
    std::fs::create_dir(&staging).unwrap();
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("m.db"), b"old main").unwrap();
    std::fs::write(destination.join("hm.db"), b"old history").unwrap();
    std::fs::create_dir(staging.join("m.db")).unwrap();
    let Error::Publication {
        rollback, recovery, ..
    } = publish(&staging, &destination).unwrap_err()
    else {
        panic!("wrong error")
    };
    assert!(!rollback.is_empty());
    let recovery = recovery.unwrap();
    assert_eq!(std::fs::read(recovery.join("m.db")).unwrap(), b"old main");
    assert_eq!(
        std::fs::read(destination.join("hm.db")).unwrap(),
        b"old history"
    );
}
