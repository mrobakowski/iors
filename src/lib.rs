use jni::objects::{AutoLocal, JFieldID, JMethodID};
use jni::signature::{JavaType, Primitive};
use jni::sys::jmethodID;
use jni::{
    descriptors::Desc,
    objects::{GlobalRef, JClass, JObject, JThrowable},
    sys::{self, jfieldID},
    JNIEnv, JNIVersion, JavaVM,
};
use once_cell::sync::OnceCell;
use std::cell::RefCell;
use std::convert::TryInto;
use std::ops::DerefMut;
use std::{collections::HashMap, convert::TryFrom, error::Error};

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

#[repr(i32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Tag {
    Pure = 0,
    Delay = 1,
    RaiseError = 2,
    Async = 3,
    Map = 4,
    FlatMap = 5,
    Attempt = 6,
}

impl TryFrom<i32> for Tag {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value < 0 || value > 5 {
            Err("invalid tag value")
        } else {
            Ok(unsafe { std::mem::transmute(value) })
        }
    }
}

static GLOBALS: OnceCell<Globals> = OnceCell::new();

struct Globals {
    class_objects: RefCell<HashMap<&'static str, GlobalRef>>,
    // fields and methods from iors
    tag: jfieldID,
    pure_value: jfieldID,
    pure_ctor: jmethodID,
    delay_thunk: jfieldID,
    raise_error_throwable: jfieldID,
    async_f: jfieldID,
    map_source: jfieldID,
    map_f: jfieldID,
    flat_map_source: jfieldID,
    flat_map_f: jfieldID,
    attempt_source: jfieldID,
    // fields and methods from scala std
    function0_apply: jmethodID,
    function1_apply: jmethodID,
}

// the jfield_ids are not thread safe normally... but they are if the backing jclass is a GlobalRef
unsafe impl Send for Globals {}

unsafe impl Sync for Globals {}

macro_rules! field_id_getters {
    ($($id:ident),*) => {$(
        fn $id<'a>() -> Result<JFieldID<'a>> {
            Ok(Globals::get().map(|g| g.$id.into()).ok_or("globals not initialized")?)
        }
    )*};
}

impl Globals {
    fn init(env: JNIEnv) -> Result<&'static Globals> {
        GLOBALS.get_or_try_init(|| {
            let mut class_objects = HashMap::new();

            macro_rules! cache_class_and_get_id {
                ($class_name:literal; $($field_or_method:ident $name:literal : $sig:literal),*) => {{
                    let class: JClass = $class_name.lookup(&env)?;
                    let class = env.new_global_ref(class)?;
                    let res = ($(
                        cache_class_and_get_id!(@do $field_or_method, class, $name, $sig)
                    ),*);

                    class_objects.insert($class_name, class);
                    res
                }};
                (@do field, $class:ident, $name:literal, $sig:literal) => {
                    env.get_field_id(&$class, $name, $sig)?.into_inner()
                };
                (@do method, $class:ident, $name:literal, $sig:literal) => {
                    env.get_method_id(&$class, $name, $sig)?.into_inner()
                }
            }

            let tag = cache_class_and_get_id!("iors/IoRs"; field "tag": "I");
            let (pure_value, pure_ctor) = cache_class_and_get_id!("iors/IoRs$Pure"; 
                field "value": "Ljava/lang/Object;",
                method "<init>": "(Ljava/lang/Object;)V"
            );
            let delay_thunk = cache_class_and_get_id!("iors/IoRs$Delay"; field "thunk": "Lscala/Function0;");
            let raise_error_throwable = cache_class_and_get_id!("iors/IoRs$RaiseError"; 
                field "throwable": "Ljava/lang/Throwable;"
            );
            let async_f = cache_class_and_get_id!("iors/IoRs$Async"; field "f": "Lscala/Function1;");
            let (map_source, map_f) = cache_class_and_get_id!("iors/IoRs$Map";
                field "source": "Liors/IoRs;",
                field "f": "Lscala/Function1;"
            );
            let (flat_map_source, flat_map_f) = cache_class_and_get_id!("iors/IoRs$FlatMap";
                field "source": "Liors/IoRs;",
                field "f": "Lscala/Function1;"
            );
            let attempt_source = cache_class_and_get_id!("iors/IoRs$Attempt"; field "source": "Liors/IoRs;");

            let function0_apply = cache_class_and_get_id!("scala/Function0"; 
                method "apply": "()Ljava/lang/Object;"
            );
            let function1_apply = cache_class_and_get_id!("scala/Function1"; 
                method "apply": "(Ljava/lang/Object;)Ljava/lang/Object;"
            );

            let class_objects = RefCell::new(class_objects);

            Ok(Globals {
                class_objects,
                tag,
                pure_value,
                pure_ctor,
                delay_thunk,
                raise_error_throwable,
                async_f,
                map_source,
                map_f,
                flat_map_source,
                flat_map_f,
                attempt_source,

                function0_apply,
                function1_apply
            })
        })
    }

    fn get() -> Option<&'static Globals> {
        GLOBALS.get()
    }

    field_id_getters! {
        tag,
        pure_value,
        delay_thunk,
        raise_error_throwable,
        async_f,
        map_source,
        map_f,
        flat_map_source,
        flat_map_f
    }
}

macro_rules! getters {
    ($($getter_name:ident $(<$l:lifetime>)? -> $res:ty, $field_id:ident, $java_type:expr, $variant:ident;)*) => {$(
        fn $getter_name$(<$l>)?(env: &$($l)? JNIEnv, io: impl Into<JObject$(<$l>)?>) -> Result<$res> {
            Ok(env
                .get_field_unchecked(io.into(), Globals::$field_id()?, $java_type)?
                .$variant()?
                .try_into()?)
        }
    )*};
}

getters! {
    get_tag<'a> -> Tag, tag, JavaType::Primitive(Primitive::Int), i;
    get_pure_value<'a> -> JObject<'a>, pure_value, JavaType::Object("Ljava/lang/Object;".into()), l;
    get_delay_thunk<'a> -> JObject<'a>, delay_thunk, JavaType::Object("Lscala/Function0;".into()), l;
    get_raise_error_throwable<'a> -> JThrowable<'a>, raise_error_throwable, JavaType::Object("Ljava/lang/Throwable;".into()), l;
    get_async_f<'a> -> JObject<'a>, async_f, JavaType::Object("Lscala/Function1;".into()), l;
    get_map_source<'a> -> JObject<'a>, map_source, JavaType::Object("Liors/IoRs;".into()), l;
    get_map_f<'a> -> JObject<'a>, map_f, JavaType::Object("Lscala/Function1;".into()), l;
    get_flat_map_source<'a> -> JObject<'a>, flat_map_source, JavaType::Object("Liors/IoRs;".into()), l;
    get_flat_map_f<'a> -> JObject<'a>, flat_map_f, JavaType::Object("Lscala/Function1;".into()), l;
}

fn call_function0<'a>(env: &'a JNIEnv, f: impl Into<JObject<'a>>) -> Result<JObject<'a>> {
    Ok(env
        .call_method_unchecked(
            f.into(),
            JMethodID::from(
                Globals::get()
                    .ok_or("globals not initialized")?
                    .function0_apply,
            ),
            JavaType::Object("Ljava/lang/Object;".into()),
            &[],
        )?
        .l()?)
}

fn call_function1<'a, 'b>(
    env: &'a JNIEnv,
    f: impl Into<JObject<'a>>,
    x: impl Into<JObject<'b>>,
) -> Result<JObject<'a>> {
    Ok(env
        .call_method_unchecked(
            f.into(),
            JMethodID::from(
                Globals::get()
                    .ok_or("globals not initialized")?
                    .function1_apply,
            ),
            JavaType::Object("Ljava/lang/Object;".into()),
            &[x.into().into()],
        )?
        .l()?)
}

#[no_mangle]
extern "system" fn JNI_OnLoad(jvm: *mut sys::JavaVM, _reserved: *const ()) -> sys::jint {
    let jvm = unsafe { JavaVM::from_raw(jvm) }.unwrap();
    let env = jvm.get_env().expect("Could not get the JNI environment");

    // we must make global refs to all the relevant class objects if we want to cache their
    // field_ids and method_ids forever
    Globals::init(env).expect("Could not initialize the globals");

    JNIVersion::V8.into()
}

#[no_mangle]
extern "system" fn JNI_OnUnload(jvm: *mut sys::JavaVM, _reserved: *const ()) {
    let _jvm = unsafe { JavaVM::from_raw(jvm) }.unwrap();
    if let Some(globals) = GLOBALS.get() {
        std::mem::take(globals.class_objects.borrow_mut().deref_mut());
    }
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

enum Bind {
    Map(GlobalRef),
    FlatMap(GlobalRef),
    Attempt,
}

fn right<'a>(env: &'a JNIEnv, o: JObject<'_>) -> Result<JObject<'a>> {
    Ok(env
        .call_static_method(
            "scala/util/Right",
            "apply",
            "(Ljava/lang/Object;)Lscala/util/Right;",
            &[o.into()],
        )?
        .l()?)
}

fn pure<'a>(env: &'a JNIEnv, o: JObject<'_>) -> Result<JObject<'a>> {
    let globals = Globals::get().ok_or("globals not initialized")?;

    Ok(env.new_object_unchecked(
        globals
            .class_objects
            .borrow()
            .get("iors/IoRs$Pure")
            .ok_or("no class for Pure")?,
        globals.pure_ctor.into(),
        &[o.into()],
    )?)
}

#[export_name = "Java_iors_IoRs_unsafeRunAsync"]
extern "system" fn eval_loop(env: JNIEnv, io: JObject, callback: JObject) {
    let mut current = env.auto_local(io);
    let mut stack = vec![];

    let next_bind = |stack: &mut Vec<Bind>| {
        let mut last = stack.pop();
        while let Some(Bind::Attempt) = last {
            last = stack.pop();
        }
        last
    };

    let next_handler = |stack: &mut Vec<Bind>| {
        let mut last = stack.pop();
        while let Some(Bind::Map(_)) | Some(Bind::FlatMap(_)) = last {
            last = stack.pop();
        }
        last
    };

    loop {
        let tag = get_tag(&env, &current).unwrap();
        let mut unwrapped_value = JObject::null();
        let mut has_unwrapped_value = false;
        match tag {
            Tag::Pure => {
                unwrapped_value = get_pure_value(&env, &current).unwrap();
                has_unwrapped_value = true;
            }
            Tag::Delay => {
                let thunk = get_delay_thunk(&env, &current).unwrap();
                // todo: error handling
                unwrapped_value = call_function0(&env, thunk).unwrap();
                has_unwrapped_value = true;
            }
            Tag::RaiseError => todo!(),
            Tag::Async => todo!(),
            Tag::Map => {
                let source = get_map_source(&env, current.as_obj().into_inner()).unwrap();
                let bind = Bind::Map(
                    env.new_global_ref(get_map_f(&env, &current).unwrap())
                        .unwrap(),
                );
                stack.push(bind);
                current = env.auto_local(source);
            }
            Tag::FlatMap => {
                let source = get_flat_map_source(&env, current.as_obj().into_inner()).unwrap();
                let bind = Bind::FlatMap(
                    env.new_global_ref(get_flat_map_f(&env, &current).unwrap())
                        .unwrap(),
                );
                stack.push(bind);
                current = env.auto_local(source);
            }
            Tag::Attempt => todo!(),
        }

        if has_unwrapped_value {
            match next_bind(&mut stack) {
                None => {
                    call_function1(&env, callback, right(&env, unwrapped_value).unwrap()).unwrap();
                    break;
                }
                Some(Bind::Map(f)) => {
                    let new_value = call_function1(&env, &f, unwrapped_value).unwrap();
                    // todo: error handling
                    current = env.auto_local(pure(&env, new_value).unwrap());
                }
                Some(Bind::FlatMap(f)) => {
                    // todo: error handling
                    current = env.auto_local(
                        call_function1(&env, f.as_obj().into_inner(), unwrapped_value).unwrap(),
                    );
                }
                Some(Bind::Attempt) => unreachable!(),
            }
        }
    }

    println!("Hello from the eval loop!")
}
