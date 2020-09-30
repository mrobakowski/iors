#![feature(bool_to_option)]

use jni::{Executor, InitArgsBuilder, JavaVM};
use once_cell::sync::Lazy;
use std::{env, ops::Deref, path::PathBuf, process::Command, sync::Arc};

static IORS_PATH: Lazy<PathBuf> = Lazy::new(test_cdylib::build_current_project);
static JAR_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let sbt_path = env::var("SBT_PATH")
        .or_else(|_| {
            const UBUNTU_PATH: &str = "/usr/share/sbt/bin/sbt-launch.jar";
            PathBuf::from(UBUNTU_PATH)
                .is_file()
                .then(|| UBUNTU_PATH.into())
                .ok_or("sbt not found on the ubuntu installation path")
        })
        .expect("SBT_PATH must be set and pointing to the sbt jar");
    let java = env::var("JAVA_HOME").map_or("java".into(), |p| {
        let mut p = PathBuf::from(p);
        p.push("bin");
        p.push("java.exe");
        p.to_string_lossy().to_string()
    });

    Command::new(java)
        .args(&["-jar", &sbt_path, "test:assembly"])
        .current_dir("./iors-jvm")
        .status()
        .unwrap();

    PathBuf::from("./iors-jvm/target/scala-2.13/iors-jvm-test-0.1.jar")
});

#[test]
fn jni_works() {
    let lib_path = format!(
        "-Djava.library.path={}",
        IORS_PATH.parent().unwrap().display(),
    );
    let jar_path = format!("-Djava.class.path={}", JAR_PATH.deref().display());

    let jvm = Arc::new(
        JavaVM::new(
            InitArgsBuilder::new()
                .option(&lib_path)
                .option(&jar_path)
                .option("-Xcheck:jni")
                .build()
                .unwrap(),
        )
        .unwrap(),
    );
    let executor = Executor::new(jvm);

    let res = executor
        .with_attached(|env| {
            env.call_static_method("iors/IoRsTests", "itWorks", "()I", &[])
                .unwrap()
                .i()
        })
        .unwrap();

    assert_eq!(res, 111);
}

#[test]
fn expensive_stuff_for_profiling() {
    let lib_path = format!(
        "-Djava.library.path={}",
        IORS_PATH.parent().unwrap().display(),
    );
    let jar_path = format!("-Djava.class.path={}", JAR_PATH.deref().display());

    let jvm = Arc::new(
        JavaVM::new(
            InitArgsBuilder::new()
                .option(&lib_path)
                .option(&jar_path)
                .build()
                .unwrap(),
        )
        .unwrap(),
    );
    let executor = Executor::new(jvm);

    executor
        .with_attached(|env| {
            env.call_static_method("iors/IoRsTests", "expensive", "()V", &[])
                .unwrap()
                .v()
        })
        .unwrap();
}
