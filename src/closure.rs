use crate::Result;
use jni::objects::JValue;
use jni::{objects::JObject, JNIEnv};
use std::sync::MutexGuard;

// todo: naming
// todo: the *_rust_field methods are doing a lot of lookups :/ we may want to do this ourselves
//   and add caching

type Closure = Option<Box<dyn FnOnce(JNIEnv, JObject) + Send + Sync + 'static>>;

#[export_name = "Java_iors_IoRs_00024RustClosure_doApply"]
extern "system" fn apply_rust_closure(env: JNIEnv, this: JObject, argument: JObject) {
    let mut guard: MutexGuard<Closure> = env.get_rust_field(this, "nativePointer").unwrap();
    let closure = guard.take();
    drop(guard);
    if let Some(closure) = closure {
        closure(env, argument);
    }
}

#[export_name = "Java_iors_IoRs_00024RustClosure_finalize"]
extern "system" fn drop_rust_closure(env: JNIEnv, this: JObject) {
    let c: Closure = env.take_rust_field(this, "nativePointer").unwrap();
    drop(c);
}

pub fn make_rust_closure<'a>(
    env: &'a JNIEnv<'a>,
    f: impl FnOnce(JNIEnv, JObject) + Send + Sync + 'static,
) -> Result<JObject> {
    let boxed = Some(Box::new(f) as Box<dyn FnOnce(JNIEnv, JObject) + Send + Sync + 'static>);
    // todo: cache the ctor maybe?
    let obj = env.new_object("iors/IoRs$RustClosure", "(J)V", &[JValue::Long(0)])?;
    env.set_rust_field(obj, "nativePointer", boxed)?;
    Ok(obj)
}
