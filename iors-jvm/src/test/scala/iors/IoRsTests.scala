package iors

import iors.IoRs.printVersion

import scala.concurrent.ExecutionContext.Implicits.global
import scala.concurrent.Future

object IoRsTests {
  def itWorks(): Int = {

    val io = for {
      x <- IoRs.pure(42)
      _ <- IoRs {
        println("Hello from iors!")
        printVersion()
      }
      e <- IoRs.raiseError[Int](new RuntimeException("foo")).attempt
      _ <- IoRs {
        println(s"the error is: $e")
      }
      y <- IoRs.async[Int] { cb =>
        val fut = Future {
          println("Hello from scala future!")
        }
        fut.onComplete(t => cb(t.toEither.map(_ => 69)))
      }

    } yield x + y

    val res = io.unsafeRunSync()
    println(s"The result of the io is $res")

    res
  }
}
