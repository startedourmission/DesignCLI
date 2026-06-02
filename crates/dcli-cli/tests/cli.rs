//! `dx` CLI 통합 테스트 — 바이너리를 실제로 실행해 verb·--json·--dry-run·결정성 검증.

use std::path::{Path, PathBuf};
use std::process::Command;

fn dx() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dx"))
}

/// 테스트별 고유 임시 폴더(병렬 테스트 충돌 방지). 빌드 OUT_DIR 하위에 만든다.
fn tmp(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("dx_test_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn run(args: &[&str]) -> (String, String, bool) {
    let out = dx().args(args).output().expect("dx 실행 실패");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

fn doc_arg(dir: &Path) -> String {
    dir.to_string_lossy().to_string()
}

#[test]
fn create_add_export_roundtrip() {
    let dir = tmp("roundtrip");
    let d = doc_arg(&dir);

    let (_, _, ok) = run(&["--doc", &d, "doc", "create", "--w", "64", "--h", "64"]);
    assert!(ok, "doc create 실패");
    assert!(dir.join("doc.json").is_file());

    let (_, _, ok) = run(&["--doc", &d, "layer", "add", "--name", "bg", "--fill", "255,0,0,255"]);
    assert!(ok);
    assert!(dir.join("pixels/0.bin").is_file());

    let png = dir.join("out.png");
    let (_, _, ok) = run(&["--doc", &d, "export", "png", &png.to_string_lossy()]);
    assert!(ok);
    assert!(png.is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn json_output_is_parseable() {
    let dir = tmp("json");
    let d = doc_arg(&dir);
    run(&["--doc", &d, "doc", "create", "--w", "8", "--h", "8"]);
    run(&["--doc", &d, "layer", "add", "--name", "a", "--fill", "1,2,3,255"]);

    let (stdout, _, ok) = run(&["--doc", &d, "--json", "doc", "info"]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect("유효한 JSON 아님");
    assert_eq!(v["w"], 8);
    assert_eq!(v["layers"], 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dry_run_does_not_persist() {
    let dir = tmp("dryrun");
    let d = doc_arg(&dir);
    run(&["--doc", &d, "doc", "create", "--w", "8", "--h", "8"]);

    // dry-run으로 레이어 추가 → applied:false, 실제 저장 안 됨.
    let (stdout, _, ok) = run(&[
        "--doc", &d, "--json", "--dry-run", "layer", "add", "--name", "x", "--fill", "0,0,0,255",
    ]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["applied"], false);

    // 레이어 수는 여전히 0.
    let (stdout, _, _) = run(&["--doc", &d, "--json", "doc", "info"]);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["layers"], 0);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_layer_errors_to_stderr_with_nonzero_exit() {
    let dir = tmp("err");
    let d = doc_arg(&dir);
    run(&["--doc", &d, "doc", "create", "--w", "8", "--h", "8"]);

    let (stdout, stderr, ok) = run(&["--doc", &d, "--json", "layer", "get", "999"]);
    assert!(!ok, "없는 레이어 조회는 exit!=0 이어야");
    assert!(stdout.trim().is_empty(), "에러 시 stdout은 비어야(데이터 없음)");
    assert!(stderr.contains("error") || stderr.contains("999"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_is_deterministic() {
    let dir = tmp("det");
    let d = doc_arg(&dir);
    run(&["--doc", &d, "doc", "create", "--w", "32", "--h", "32"]);
    run(&["--doc", &d, "layer", "add", "--name", "bg", "--fill", "200,100,50,255"]);
    run(&["--doc", &d, "layer", "add", "--name", "top", "--fill", "100,100,100,200"]);
    run(&["--doc", &d, "blend", "set", "1", "multiply"]);

    let p1 = dir.join("a.png");
    let p2 = dir.join("b.png");
    run(&["--doc", &d, "export", "png", &p1.to_string_lossy()]);
    run(&["--doc", &d, "export", "png", &p2.to_string_lossy()]);

    let a = std::fs::read(&p1).unwrap();
    let b = std::fs::read(&p2).unwrap();
    assert_eq!(a, b, "같은 문서의 export PNG가 비결정적");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_load_preserves_structure_and_pixels() {
    let dir = tmp("persist");
    let d = doc_arg(&dir);
    run(&["--doc", &d, "doc", "create", "--w", "16", "--h", "16"]);
    run(&["--doc", &d, "layer", "add", "--name", "bg", "--fill", "10,20,30,255"]);

    // 첫 export.
    let p1 = dir.join("first.png");
    run(&["--doc", &d, "export", "png", &p1.to_string_lossy()]);

    // 새 프로세스가 load 후 다시 export(디스크 라운드트립) → 동일해야.
    let p2 = dir.join("reloaded.png");
    run(&["--doc", &d, "export", "png", &p2.to_string_lossy()]);

    assert_eq!(std::fs::read(&p1).unwrap(), std::fs::read(&p2).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

fn count_bins(dir: &std::path::Path) -> usize {
    let pix = dir.join("pixels");
    std::fs::read_dir(&pix)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "bin").unwrap_or(false))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn delete_then_save_removes_orphan_bin() {
    // 검증 #1 회귀 가드: 레이어 삭제 후 저장하면 그 픽셀 .bin이 디스크에서 사라져야.
    let dir = tmp("orphan");
    let d = doc_arg(&dir);
    run(&["--doc", &d, "doc", "create", "--w", "8", "--h", "8"]);
    run(&["--doc", &d, "layer", "add", "--name", "a", "--fill", "1,2,3,255"]);
    run(&["--doc", &d, "layer", "add", "--name", "b", "--fill", "4,5,6,255"]);
    assert_eq!(count_bins(&dir), 2, "레이어 2개 → .bin 2개");

    // 레이어 하나 삭제(삭제는 자동 저장됨).
    run(&["--doc", &d, "layer", "delete", "0"]);
    assert_eq!(count_bins(&dir), 1, "삭제 후 orphan .bin 없어야(1개만)");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_is_atomic_no_partial_dir() {
    // 정상 저장 후 임시/백업 잔여 디렉토리가 남지 않아야(원자적 교체).
    // 격리 부모 폴더 안에 문서를 둬 다른 테스트의 .tmp-/.bak과 섞이지 않게.
    let parent = tmp("atomic");
    std::fs::create_dir_all(&parent).unwrap();
    let dir = parent.join("doc.dxdoc");
    let d = doc_arg(&dir);
    run(&["--doc", &d, "doc", "create", "--w", "8", "--h", "8"]);
    run(&["--doc", &d, "layer", "add", "--name", "a", "--fill", "1,2,3,255"]);

    // 격리 부모에 .tmp-/.bak 잔여물이 없어야.
    let leftovers: Vec<_> = std::fs::read_dir(&parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.contains(".tmp-") || n.ends_with(".bak")
        })
        .collect();
    assert!(leftovers.is_empty(), "원자적 save 후 잔여 디렉토리: {leftovers:?}");
    assert!(dir.join("doc.json").is_file(), "doc.json 존재해야");

    let _ = std::fs::remove_dir_all(&parent);
}
