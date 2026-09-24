@file:OptIn(kotlin.ExperimentalUnsignedTypes::class)

package com.boltffi.demo

fun blob(ratio: Double = 0.5, weight: Float? = 1.5f) = Blob(
    7u,
    byteArrayOf(1, 2, 3),
    uintArrayOf(4u, 5u),
    byteArrayOf(6),
    listOf(byteArrayOf(7), byteArrayOf(8, 9)),
    ratio,
    weight,
)

fun main() {
    check(blob() == blob())
    check(blob().hashCode() == blob().hashCode())
    check(blob() != blob().copy(payload = byteArrayOf(1, 2, 4)))
    check(blob() != blob().copy(samples = uintArrayOf(4u)))
    check(blob() != blob().copy(checksum = null))
    check(blob().copy(checksum = null) == blob().copy(checksum = null))
    check(blob() != blob().copy(chunks = listOf(byteArrayOf(7), byteArrayOf(8))))
    check(blob() != blob().copy(chunks = listOf(byteArrayOf(7))))
    check(blob(ratio = Double.NaN) == blob(ratio = Double.NaN))
    check(blob(ratio = 0.0) != blob(ratio = -0.0))
    check(blob(weight = null) == blob(weight = null))
    check(blob(weight = null) != blob())
    check(blob() != Label("blob") as Any)
    check(Blob.fromByteArray(blob().toByteArray()) == blob())
    check(setOf(blob(), blob()).size == 1)

    check(Frame.Data(byteArrayOf(1)) == Frame.Data(byteArrayOf(1)))
    check(Frame.Data(byteArrayOf(1)).hashCode() == Frame.Data(byteArrayOf(1)).hashCode())
    check(Frame.Data(byteArrayOf(1)) != Frame.Data(byteArrayOf(2)))
    check(Frame.Tagged(1u, longArrayOf(2)) == Frame.Tagged(1u, longArrayOf(2)))
    check(Frame.Tagged(1u, longArrayOf(2)) != Frame.Tagged(2u, longArrayOf(2)))
    check(Frame.fromByteArray(Frame.Data(byteArrayOf(1)).toByteArray()) == Frame.Data(byteArrayOf(1)))

    check(BlobError("failed", byteArrayOf(1)) == BlobError("failed", byteArrayOf(1)))
    check(BlobError("failed", byteArrayOf(1)) != BlobError("failed", byteArrayOf(2)))
}
