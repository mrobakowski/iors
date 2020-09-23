use jni::{Executor, InitArgsBuilder, JavaVM};
use once_cell::sync::Lazy;
use std::{path::PathBuf, sync::Arc};

static IORS_PATH: Lazy<PathBuf> = Lazy::new(|| test_cdylib::build_current_project());

#[test]
fn jni_works() {
    let opt = format!(
        "-Djava.library.path={}",
        IORS_PATH.parent().unwrap().to_string_lossy(),
    );
    dbg!(&opt);
    let jvm = Arc::new(
        JavaVM::new(
            InitArgsBuilder::new()
                .option(&opt)
                .option("-Djava.class.path=./iors-jvm/target/scala-2.13/iors-jvm-assembly-0.1.jar")
                .build()
                .unwrap(),
        )
        .unwrap(),
    );
    let executor = Executor::new(jvm);

    executor
        .with_attached(|env| {
            env.call_static_method("iors/IoRs", "printVersion", "()V", &[])
                .unwrap()
                .v()
                .unwrap();

            Ok(())
        })
        .unwrap();
}
