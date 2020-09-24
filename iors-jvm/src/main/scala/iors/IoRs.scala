package iors
import IoRs.Tag

import scala.concurrent.ExecutionContext.Implicits.global
import scala.concurrent.Future

abstract sealed class IoRs[+A](val tag: Tag) {
  @native def unsafeRunAsync(cb: Either[Throwable, A] => ())

  def map[B](f: A => B): IoRs[B] = IoRs.Map(this, f)
  def flatMap[B](f: A => IoRs[B]): IoRs[B] = IoRs.FlatMap(this, f)
}

object IoRs {
  System.loadLibrary("iors")
  @native def printVersion()

  def main(args: Array[String]): () = {
    val io = for {
      x <- IoRs.pure(42)
      _ <- IoRs {
        println("Hello from iors!")
        printVersion()
      }
      y <- IoRs.async[Int] { cb =>
        val fut = Future { println("Hello from scala future!") }
        fut.onComplete(t => cb(t.toEither.map(_ => 69)))
      }

    } yield x + y

    io.unsafeRunAsync { res =>
      println(s"The result of the io is $res")
    }
  }

  def pure[A](value: A): IoRs[A] = IoRs.Pure(value)
  def delay[A](body: => A): IoRs[A] = IoRs.Delay(() => body)
  def apply[A](body: => A): IoRs[A] = delay(body)
  def raiseError[A](throwable: Throwable): IoRs[A] = IoRs.RaiseError(throwable)
  def async[A](f: (Either[Throwable, A] => ()) => ()): IoRs[A] = IoRs.Async(f)

  private[iors] case class Tag(underlying: Int) extends AnyVal
  private[iors] object Tag {
    val Pure: Tag = Tag(0)
    val Delay: Tag = Tag(1)
    val RaiseError: Tag = Tag(2)
    val Async: Tag = Tag(3)
    val Map: Tag = Tag(4)
    val FlatMap: Tag = Tag(5)
    val Attempt: Tag = Tag(6)
  }

  private[iors] case class Pure[+A](value: A) extends IoRs[A](Tag.Pure)
  private[iors] case class Delay[+A](thunk: () => A) extends IoRs[A](Tag.Delay)
  private[iors] case class RaiseError(throwable: Throwable) extends IoRs[Nothing](Tag.RaiseError)
  private[iors] case class Async[+A](f: (Either[Throwable, A] => ()) => ()) extends IoRs[A](Tag.Async)
  private[iors] case class Map[E, +A](source: IoRs[E], f: E => A) extends IoRs[A](Tag.Map)
  private[iors] case class FlatMap[E, +A](source: IoRs[E], f: E => IoRs[A]) extends IoRs[A](Tag.FlatMap)
  private[iors] case class Attempt[+A](source: IoRs[A]) extends IoRs[Either[Throwable, A]](Tag.Attempt)
}
