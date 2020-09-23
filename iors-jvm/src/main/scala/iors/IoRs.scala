package iors

object IoRs {
  System.loadLibrary("iors")
  @native def printVersion()

  def main(args: Array[String]): Unit = {
    printVersion()
  }
}
