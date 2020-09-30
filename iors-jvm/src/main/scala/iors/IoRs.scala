package iors

import java.util.concurrent.ArrayBlockingQueue

import iors.IoRs.Tag

import scala.annotation.unused

abstract sealed class IoRs[+A] private(private val tag: Tag) {
  @native def unsafeRunAsync(/* actually used, but the lint fires here */ @unused cb: Either[Throwable, A] => ()): Unit

  def unsafeRunSync(): Either[Throwable, A] = {
    val queue = new ArrayBlockingQueue[Either[Throwable, A]](1)
    unsafeRunAsync(queue.put)
    queue.take()
  }

  def map[B](f: A => B): IoRs[B] = IoRs.Map(this, f)

  def flatMap[B](f: A => IoRs[B]): IoRs[B] = IoRs.FlatMap(this, f)

  def attempt: IoRs[Either[Throwable, A]] = IoRs.Attempt(this)
}

object IoRs {
  System.loadLibrary("iors")

  @native def printVersion(): Unit

  def pure[A](value: A): IoRs[A] = IoRs.Pure(value)

  def delay[A](body: => A): IoRs[A] = IoRs.Delay(() => body)

  def apply[A](body: => A): IoRs[A] = delay(body)

  def raiseError[A](throwable: Throwable): IoRs[A] = IoRs.RaiseError(throwable)

  def async[A](f: (Either[Throwable, A] => ()) => ()): IoRs[A] = IoRs.Async(f)

  def fromEither[A](either: Either[Throwable, A]): IoRs[A] = {
    either match {
      case Left(throwable) => RaiseError(throwable)
      case Right(value) => Pure(value)
    }
  }

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

  private[iors] final case class Pure[+A](value: A) extends IoRs[A](Tag.Pure)

  private[iors] final case class Delay[+A](thunk: () => A) extends IoRs[A](Tag.Delay)

  private[iors] final case class RaiseError(throwable: Throwable) extends IoRs[Nothing](Tag.RaiseError)

  private[iors] final case class Async[+A](f: (Either[Throwable, A] => ()) => ()) extends IoRs[A](Tag.Async)

  private[iors] final case class Map[E, +A](source: IoRs[E], f: E => A) extends IoRs[A](Tag.Map)

  private[iors] final case class FlatMap[E, +A](source: IoRs[E], f: E => IoRs[A]) extends IoRs[A](Tag.FlatMap)

  private[iors] final case class Attempt[+A](source: IoRs[A]) extends IoRs[Either[Throwable, A]](Tag.Attempt)

  private[iors] final class FfiClosure[-A](private var nativePointer: Long) extends (A => ()) {
    @native def doApply(/* actually used, but the lint fires here */ @unused v: A): Unit

    override def apply(v: A): Unit = doApply(v)

    @native override def finalize(): Unit
  }

}
