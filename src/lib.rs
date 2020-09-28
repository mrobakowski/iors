use crate::closure::make_rust_closure;
use jni::{
    descriptors::Desc,
    objects::{GlobalRef, JClass, JFieldID, JMethodID, JObject, JStaticMethodID, JThrowable},
    signature::{JavaType, Primitive},
    sys::{self, jclass, jfieldID, jmethodID},
    JNIEnv, JNIVersion, JavaVM,
};
use once_cell::sync::OnceCell;
use std::{
    cell::RefCell,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    error::Error,
    ops::DerefMut,
};

mod closure;

type Result<T, E = Box<dyn Error + 'static>> = std::result::Result<T, E>;

enum JvmResult<'a, T> {
    Value(T),
    Exception(JThrowable<'a>),
}

trait ResultExt<T> {
    fn check_exception<'a>(self, env: &'a JNIEnv<'a>) -> Result<JvmResult<'a, T>>;
}

impl<T> ResultExt<T> for Result<T> {
    fn check_exception<'a>(self, env: &'a JNIEnv<'a>) -> Result<JvmResult<'a, T>> {
        match self {
            Ok(t) => Ok(JvmResult::Value(t)),
            Err(e) => match e.downcast::<jni::errors::Error>() {
                Ok(jni_err) => {
                    if let jni::errors::ErrorKind::JavaException = jni_err.kind() {
                        let exc = env.exception_occurred()?;
                        env.exception_clear()?;
                        Ok(JvmResult::Exception(exc))
                    } else {
                        Err(jni_err.into())
                    }
                }
                Err(e) => Err(e),
            },
        }
    }
}

impl<T> ResultExt<T> for jni::errors::Result<T> {
    fn check_exception<'a>(self, env: &'a JNIEnv<'a>) -> Result<JvmResult<'a, T>> {
        match self {
            Ok(t) => Ok(JvmResult::Value(t)),
            Err(e) => {
                if let jni::errors::ErrorKind::JavaException = e.kind() {
                    let exc = env.exception_occurred()?;
                    env.exception_clear()?;
                    return Ok(JvmResult::Exception(exc));
                }

                Err(e.into())
            }
        }
    }
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Tag {
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
        if value < 0 || value > 6 {
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
    iors_class: jclass,
    iors_from_either: jmethodID,
    pure_value: jfieldID,
    pure_ctor: jmethodID,
    pure_class: jclass,
    delay_thunk: jfieldID,
    raise_error_throwable: jfieldID,
    raise_error_class: jclass,
    raise_error_ctor: jmethodID,
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
                };
                (@do static_method, $class:ident, $name:literal, $sig:literal) => {
                    env.get_static_method_id(&$class, $name, $sig)?.into_inner()
                }
            }

            let (tag, iors_from_either) = cache_class_and_get_id!("iors/IoRs"; 
                field "tag": "I",
                static_method "fromEither": "(Lscala/util/Either;)Liors/IoRs;"
            );
            let (pure_value, pure_ctor) = cache_class_and_get_id!("iors/IoRs$Pure"; 
                field "value": "Ljava/lang/Object;",
                method "<init>": "(Ljava/lang/Object;)V"
            );
            let delay_thunk = cache_class_and_get_id!("iors/IoRs$Delay"; field "thunk": "Lscala/Function0;");
            let (raise_error_throwable, raise_error_ctor) = cache_class_and_get_id!("iors/IoRs$RaiseError"; 
                field "throwable": "Ljava/lang/Throwable;",
                method "<init>": "(Ljava/lang/Throwable;)V"
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

            let pure_class = class_objects.get("iors/IoRs$Pure").ok_or("no class for Pure")?.as_obj().into_inner();
            let iors_class = class_objects.get("iors/IoRs").ok_or("no class for IoRs")?.as_obj().into_inner();
            let raise_error_class = class_objects.get("iors/IoRs$RaiseError").ok_or("no class for RaiseError")?.as_obj().into_inner();

            let class_objects = RefCell::new(class_objects);

            Ok(Globals {
                class_objects,
                tag,
                iors_class,
                iors_from_either,
                pure_value,
                pure_ctor,
                pure_class,
                delay_thunk,
                raise_error_throwable,
                raise_error_ctor,
                raise_error_class,
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
        flat_map_f,
        attempt_source
    }
}

macro_rules! getters {
    ($($getter_name:ident $(<$l:lifetime>)? -> $res:ty, $field_id:ident, $java_type:expr, $variant:ident;)*) => {$(
        fn $getter_name<$($l,)? 'x>(env: &$($l)? JNIEnv, io: impl Into<JObject<'x>>) -> Result<$res> {
            Ok(env
                .get_field_unchecked(io.into().into_inner(), Globals::$field_id()?, $java_type)?
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
    get_attempt_source<'a> -> JObject<'a>, attempt_source, JavaType::Object("Liors/IoRs;".into()), l;
}

fn call_function0<'a, 'f>(
    env: &'a JNIEnv,
    f: impl Into<JObject<'f>>,
) -> Result<JvmResult<'a, JObject<'a>>> {
    let res = (|| -> Result<_> {
        Ok(env
            .call_method_unchecked(
                f.into().into_inner(),
                JMethodID::from(
                    Globals::get()
                        .ok_or("globals not initialized")?
                        .function0_apply,
                ),
                JavaType::Object("Ljava/lang/Object;".into()),
                &[],
            )?
            .l()?)
    })();
    Ok(res.check_exception(env)?)
}

fn call_function1<'a, 'f, 'x>(
    env: &'a JNIEnv,
    f: impl Into<JObject<'f>>,
    x: impl Into<JObject<'x>>,
) -> Result<JvmResult<'a, JObject<'a>>> {
    let res: Result<_> = (|| -> Result<_> {
        Ok(env
            .call_method_unchecked(
                f.into().into_inner(),
                JMethodID::from(
                    Globals::get()
                        .ok_or("globals not initialized")?
                        .function1_apply,
                ),
                JavaType::Object("Ljava/lang/Object;".into()),
                &[x.into().into()],
            )?
            .l()?)
    })();
    Ok(res.check_exception(env)?)
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
extern "system" fn print_version(_env: JNIEnv, _this: JObject) {
    println!("iors ver. {}", env!("CARGO_PKG_VERSION"));
}

// todo: maybe it's worth to have two versions of this - with GlobalRefs and with AutoRefs
enum Bind {
    Map(GlobalRef),
    FlatMap(GlobalRef),
    Attempt,
}

// todo: cache left and right
fn right<'a>(env: &'a JNIEnv, o: JObject) -> Result<JObject<'a>> {
    Ok(env
        .call_static_method(
            "scala/util/Right",
            "apply",
            "(Ljava/lang/Object;)Lscala/util/Right;",
            &[o.into()],
        )?
        .l()?)
}

fn left<'a>(env: &'a JNIEnv, o: JObject) -> Result<JObject<'a>> {
    Ok(env
        .call_static_method(
            "scala/util/Left",
            "apply",
            "(Ljava/lang/Object;)Lscala/util/Left;",
            &[o.into()],
        )?
        .l()?)
}

fn pure<'a>(env: &'a JNIEnv, o: JObject) -> Result<JObject<'a>> {
    let globals = Globals::get().ok_or("globals not initialized")?;

    Ok(env.new_object_unchecked(
        JClass::from(globals.pure_class),
        globals.pure_ctor.into(),
        &[o.into()],
    )?)
}

fn raise_error<'a>(env: &'a JNIEnv, o: JThrowable) -> Result<JObject<'a>> {
    let globals = Globals::get().ok_or("globals not initialized")?;

    Ok(env.new_object_unchecked(
        JClass::from(globals.raise_error_class),
        globals.raise_error_ctor.into(),
        &[o.into()],
    )?)
}

fn iors_from_either<'a, 'b>(env: &'a JNIEnv<'a>, either: JObject<'b>) -> Result<JObject<'a>> {
    let globals = Globals::get().ok_or("globals not initialized")?;

    Ok(env
        .call_static_method_unchecked(
            JClass::from(globals.iors_class),
            JStaticMethodID::from(globals.iors_from_either),
            JavaType::Object("Liors/IoRs;".into()),
            &[either.into()],
        )?
        .l()?)
}

#[export_name = "Java_iors_IoRs_unsafeRunAsync"]
extern "system" fn eval_loop(env: JNIEnv, io: JObject, callback: JObject) {
    let callback = env.new_global_ref(callback).unwrap();
    eval_loop_with_stack(env, io, callback, vec![]);
}

fn eval_loop_with_stack(env: JNIEnv, io: JObject, callback: GlobalRef, mut stack: Vec<Bind>) {
    let mut current = env.auto_local(io);

    loop {
        // todo: loop jni frame that returns new current
        let tag = get_tag(&env, &current).unwrap();
        let mut unwrapped_value = None;
        match tag {
            Tag::Pure => {
                unwrapped_value = Some(get_pure_value(&env, &current).unwrap());
            }
            Tag::Delay => {
                let thunk = get_delay_thunk(&env, &current).unwrap();
                let thunk_res = call_function0(&env, thunk).unwrap();
                match thunk_res {
                    JvmResult::Value(value) => {
                        unwrapped_value = Some(value);
                    }
                    JvmResult::Exception(exc) => {
                        current = env.auto_local(raise_error(&env, exc).unwrap());
                    }
                }
            }
            Tag::RaiseError => {
                let exc = get_raise_error_throwable(&env, &current).unwrap();
                let wrapped = left(&env, exc.into()).unwrap();

                let mut attempt_bind = stack.pop();
                while let Some(Bind::Map(_)) | Some(Bind::FlatMap(_)) = attempt_bind {
                    attempt_bind = stack.pop()
                }

                match attempt_bind {
                    None => {
                        // we've reached the top of the callstack, let's fire the callback
                        // we ignore the result of that so we don't panic on java exception
                        let _ = call_function1(&env, &callback, wrapped);
                        return;
                    }
                    Some(_) => {
                        // we've reached an attempt frame, so the next frames expect Left with
                        // the error
                        current = env.auto_local(pure(&env, wrapped).unwrap());
                    }
                }
            }
            Tag::Async => {
                let f = get_async_f(&env, &current).unwrap();
                let async_cb = make_rust_closure(&env, move |env, async_result| {
                    // the JObject dance is due to the borrowchk, but I'm pretty sure this is safe
                    let io =
                        JObject::from(iors_from_either(&env, async_result).unwrap().into_inner());
                    eval_loop_with_stack(env, io, callback, stack)
                })
                .unwrap();
                match call_function1(&env, f, async_cb).unwrap() {
                    JvmResult::Value(_) => {}
                    JvmResult::Exception(_) => {}
                }
                return;
            }
            Tag::Map => {
                let source = get_map_source(&env, &current).unwrap();
                let bind = Bind::Map(
                    env.new_global_ref(get_map_f(&env, &current).unwrap())
                        .unwrap(),
                );
                stack.push(bind);
                current = env.auto_local(source);
            }
            Tag::FlatMap => {
                let source = get_flat_map_source(&env, &current).unwrap();
                let bind = Bind::FlatMap(
                    env.new_global_ref(get_flat_map_f(&env, &current).unwrap())
                        .unwrap(),
                );
                stack.push(bind);
                current = env.auto_local(source);
            }
            Tag::Attempt => {
                let source = get_attempt_source(&env, &current).unwrap();
                stack.push(Bind::Attempt);
                current = env.auto_local(source);
            }
        }

        if let Some(unwrapped_value) = unwrapped_value {
            match stack.pop() {
                None => {
                    // we ignore the result of that so we don't panic on java exception
                    let _ = call_function1(&env, &callback, right(&env, unwrapped_value).unwrap());
                    break;
                }
                Some(Bind::Map(f)) => {
                    let f_res = call_function1(&env, &f, unwrapped_value).unwrap();
                    let next_io = match f_res {
                        // f: value -> value, so we need to wrap in a pure
                        JvmResult::Value(new_value) => pure(&env, new_value).unwrap(),
                        JvmResult::Exception(exc) => raise_error(&env, exc).unwrap(),
                    };
                    current = env.auto_local(next_io);
                }
                Some(Bind::FlatMap(f)) => {
                    let f_res = call_function1(&env, &f, unwrapped_value).unwrap();
                    let next_io = match f_res {
                        // f: value -> io, so we just pass it along
                        JvmResult::Value(new_value) => new_value,
                        JvmResult::Exception(exc) => raise_error(&env, exc).unwrap(),
                    };
                    current = env.auto_local(next_io);
                }
                Some(Bind::Attempt) => {
                    current =
                        env.auto_local(pure(&env, right(&env, unwrapped_value).unwrap()).unwrap());
                }
            }
        }
    }
}
