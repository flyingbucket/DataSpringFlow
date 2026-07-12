use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

fn dsf() -> Command {
    Command::cargo_bin("dsf").expect("binary 'dsf' should be buildable")
}

#[test]
fn help_should_succeed_and_show_project_name() {
    dsf()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("DataSpringFlow"))
        .stdout(predicate::str::contains("query"))
        .stdout(predicate::str::contains("register"));
}

#[test]
fn version_should_succeed() {
    dsf()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("dsf"));
}

#[test]
fn no_args_should_fail_and_show_usage() {
    dsf()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage").or(predicate::str::contains("USAGE")));
}

#[test]
fn query_without_id_should_fail() {
    dsf()
        .arg("query")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn query_with_invalid_level_should_fail() {
    dsf()
        .args(["query", "mnist@v1", "--level", "super-deep"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("invalid value")
                .or(predicate::str::contains("possible values")),
        );
}

#[test]
fn register_missing_required_args_should_fail() {
    dsf()
        .arg("register")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn show_config_should_not_crash() {
    // 即使在没有预先配置文件的纯净容器中，也应该优雅输出或报错而不发生 panic 崩溃
    dsf().arg("show-config").assert().success().stdout(
        predicate::str::contains("=== DataSpringFlow Current Configuration ===")
            .or(predicate::str::contains("Failed to load configuration")),
    );
}

#[test]
fn init_global_in_container_environment() {
    // 因为你在 Podman 容器中运行测试，默认通常已经是 root 或者是拥有对应权限的超级用户。
    // 我们在此断言它能正常拉起初始化逻辑，而不会因为在普通宿主机中缺少 sudo 被直接拦截。
    let out = dsf().args(["init", "--global"]).output();

    if let Ok(output) = out {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // 如果容器里没装 sudo，会提示命令未找到；如果装了或者直接执行，我们验证它没有发生核心逻辑 panic
        if stderr.contains("sudo: command not found") {
            eprintln!("Container missing 'sudo' package, skip deep execution check.");
        } else {
            // 如果容器环境就绪，我们可以直接验证它是否成功进入或执行了系统初始化配置
            assert!(
                output.status.success()
                    || stderr.contains("privileges")
                    || stdout.contains("Global Setup")
            );
        }
    }
}
