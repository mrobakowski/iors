# iors
`iors` (pronounce it however you want, but I like to say it like "yours") is an experimental IO runtime for Scala,
 written in Rust. Lots of JNI and other kinds of scary code.
 
 ## Performance
 Performance _sucks_ big time. 
 
 When I first started the project, I thought it _may_ outperform an optimized IO interpreter written 
 in Scala because, you know, JVM is supposed to be slow and Rust is supposed to be fast. 
 **BUT**, after thinking about this more, this will obviously inhibit some of JVM's JIT optimizations, so it can 
 actually end up _slower_. Who knows, JITs are weird. But yeah, it did end up significantly slower. Unless I measured
 something very incorrectly (which may have happened).
 
 Benchmarks are shamelessly stolen from [`cats-effect`](https://github.com/typelevel/cats-effect/tree/series/2.x/benchmarks/shared/src/main/scala/cats/effect/benchmarks).
 
 To be fair I haven't run them all, because after running just one of them 
 ([AttemptBenchmark](./iors-jvm/src/benchmark/scala/iors/AttemptBenchmark.scala)) I saw numbers around **50x worse** than
 `cats-effect`, which I based the implementation on.
 
 ## Why?
 idk
 
 for fun?
 
 ## Should I use this?
 no.
 
 ## Lessons learned
 This was my first "big" project involving JNI. Now I know that JNI is _slooooow_ if you do a lot of switching between
 native code and JVM code, like in callback-heavy code, which an IO interpreter obviously is.
 
 ## License
 MIT