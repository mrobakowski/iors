use jni::objects::JObject;
use jni::{Executor, InitArgsBuilder, JavaVM};
use once_cell::sync::Lazy;
use std::ops::Deref;
use std::process::Command;
use std::{env, path::PathBuf, sync::Arc};

static IORS_PATH: Lazy<PathBuf> = Lazy::new(test_cdylib::build_current_project);
static JAR_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let sbt_path = env::var("SBT_PATH").expect("SBT_PATH must be set and pointing to the sbt jar");
    let java = env::var("JAVA_HOME").map_or("java".into(), |p| {
        let mut p = PathBuf::from(p);
        p.push("bin");
        p.push("java.exe");
        p.to_string_lossy().to_string()
    });

    Command::new(java)
        .args(&["-jar", &sbt_path, "assembly"])
        .current_dir("./iors-jvm")
        .status()
        .unwrap();

    PathBuf::from("./iors-jvm/target/scala-2.13/iors-jvm-assembly-0.1.jar")
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

    executor
        .with_attached(|env| {
            env.call_static_method(
                "iors/IoRs",
                "main",
                "([Ljava/lang/String;)V",
                &[JObject::null().into()],
            )
            .unwrap()
            .v()
            .unwrap();

            Ok(())
        })
        .unwrap();
}
