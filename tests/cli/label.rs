//! label set 相关 ST：#409 补测。

use super::*;

/// label set：缺 color/description 拒绝；非法颜色拒绝；合法更新 + JSON。
#[test]
fn st_label_set_validation_and_update() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "lbl");
    mint(&db)
        .args(["issue", "label", "attach", &id.to_string(), "backend"])
        .assert()
        .success();
    let err = run_fail(&db, &["label", "set", "backend"]);
    assert!(err.contains("requires --color or --description"), "{err}");
    let err = run_fail(&db, &["label", "set", "backend", "--color", "red"]);
    assert!(err.contains("invalid color"), "{err}");
    let v = run_json(
        &db,
        &[
            "label",
            "set",
            "backend",
            "--color",
            "#ff0000",
            "--description",
            "b",
            "--json",
        ],
    );
    assert_eq!(v["name"], "backend");
}
