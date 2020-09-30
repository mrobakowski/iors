/*
 * Copyright (c) 2017-2019 The Typelevel Cats-effect Project Developers
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package iors

import java.util.concurrent.TimeUnit

import org.openjdk.jmh.annotations._

import scala.concurrent.ExecutionContext
import scala.concurrent.ExecutionContext.Implicits

/** To do comparative benchmarks between versions:
 *
 *     benchmarks/run-benchmark ShallowBindBenchmark
 *
 * This will generate results in `benchmarks/results`.
 *
 * Or to run the benchmark from within sbt:
 *
 *     jmh:run -i 10 -wi 10 -f 2 -t 1 iors.ShallowBindBenchmark
 *
 * Which means "10 iterations", "10 warm-up iterations", "2 forks", "1 thread".
 * Please note that benchmarks should be usually executed at least in
 * 10 iterations (as a rule of thumb), but more is better.
 */
@State(Scope.Thread)
@BenchmarkMode(Array(Mode.Throughput))
@OutputTimeUnit(TimeUnit.SECONDS)
class ShallowBindBenchmark {
  implicit val ec: ExecutionContext = Implicits.global

  @Param(Array("10000"))
  var size: Int = _

  @Benchmark
  def pure(): Int = {
    def loop(i: Int): IoRs[Int] =
      if (i < size) IoRs.pure(i + 1).flatMap(loop)
      else IoRs.pure(i)

    IoRs.pure(0)
      .flatMap(loop)
      .unsafeRunSync()
  }

  @Benchmark
  def delay(): Int = {
    def loop(i: Int): IoRs[Int] =
      if (i < size) IoRs(i + 1).flatMap(loop)
      else IoRs(i)

    IoRs(0).flatMap(loop).unsafeRunSync()
  }

  @Benchmark
  def async(): Int = {
    def loop(i: Int): IoRs[Int] =
      if (i < size) IoRs.shift.flatMap(_ => IoRs.pure(i + 1)).flatMap(loop)
      else IoRs.shift.flatMap(_ => IoRs.pure(i))

    IoRs(0).flatMap(loop).unsafeRunSync()
  }
}
