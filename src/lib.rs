use jni::objects::JObject;
use jni::{sys, JNIEnv, JNIVersion, JavaVM};

#[no_mangle]
extern "system" fn JNI_OnLoad(jvm: *mut sys::JavaVM, _reserved: *const ()) -> sys::jint {
    let jvm = unsafe { JavaVM::from_raw(jvm) };
    println!("library successfully loaded!, jvm: {:?}", jvm.is_ok());
    JNIVersion::V8.into()
}

#[no_mangle]
extern "system" fn JNI_OnUnload(jvm: *mut sys::JavaVM, _reserved: *const ()) -> sys::jint {
    let jvm = unsafe { JavaVM::from_raw(jvm) };
    println!("library unloading!, jvm: {:?}", jvm.is_ok());
    JNIVersion::V8.into()
}

#[export_name = "Java_iors_IoRs_00024_printVersion"]
extern "system" fn print_version(env: JNIEnv, this: JObject) {
    let clazz = env.get_object_class(this).unwrap();
    let name = env
        .call_method(clazz, "getName", "()Ljava/lang/String;", &[])
        .unwrap()
        .l()
        .unwrap();
    let name = env.get_string(name.into()).unwrap();
    let name = name.to_string_lossy();

    println!("iors ver. {}", env!("CARGO_PKG_VERSION"));
    println!("this has type: {}", name);
}
