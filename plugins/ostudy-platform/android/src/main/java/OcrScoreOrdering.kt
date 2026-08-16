package io.github.jferments.ostudy.platform

/** Lexicographic ordering for bounded OCR quality vectors. */
internal operator fun List<Double>.compareTo(other: List<Double>): Int {
    val common = minOf(size, other.size)
    for (index in 0 until common) {
        val comparison = this[index].compareTo(other[index])
        if (comparison != 0) return comparison
    }
    return size.compareTo(other.size)
}
