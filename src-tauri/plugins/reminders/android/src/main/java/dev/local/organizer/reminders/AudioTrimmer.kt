package dev.local.organizer.reminders

import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.media.MediaMuxer
import java.io.File
import java.nio.ByteBuffer

/**
 * Cuts the first ten seconds of any audio the device can decode into an
 * `.m4a` the notification channel can play.
 *
 * A notification sound is a jingle, not a listening session, so a picked
 * three-minute song is taken from the top and stopped at the limit; a file
 * already shorter than the limit passes through whole. Going through PCM and
 * back — decode with [MediaCodec], encode AAC-LC, mux into mp4 — is what makes
 * the input format irrelevant: mp3, m4a, ogg, wav, flac and the audio track of
 * an mp4 video all come out the same.
 */
internal object AudioTrimmer {

    /** Ten seconds: long enough to recognise, short enough for a notification. */
    const val MAX_DURATION_US = 10_000_000L

    private const val OUTPUT_BITRATE = 128_000
    private const val TIMEOUT_US = 10_000L

    fun trim(input: File, output: File) {
        val extractor = MediaExtractor()
        var decoder: MediaCodec? = null
        var encoder: MediaCodec? = null
        var muxer: MediaMuxer? = null
        var muxerStarted = false
        try {
            extractor.setDataSource(input.absolutePath)
            // In an mp4 video the audio is rarely track zero, so every track is
            // considered and the first audio one wins.
            val track = (0 until extractor.trackCount).firstOrNull { index ->
                extractor.getTrackFormat(index)
                    .getString(MediaFormat.KEY_MIME)
                    ?.startsWith("audio/") == true
            } ?: throw IllegalArgumentException("в файле нет аудиодорожки")
            extractor.selectTrack(track)

            val sourceFormat = extractor.getTrackFormat(track)
            val sourceMime = sourceFormat.getString(MediaFormat.KEY_MIME)
                ?: throw IllegalArgumentException("у аудиодорожки не указан формат")

            decoder = MediaCodec.createDecoderByType(sourceMime)
            decoder.configure(sourceFormat, null, null, 0)
            decoder.start()

            muxer = MediaMuxer(output.absolutePath, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4)

            // Taken from the decoder's *output* format once it announces one:
            // the container header can disagree with what actually comes out.
            var sampleRate = sourceFormat.intOr(MediaFormat.KEY_SAMPLE_RATE, 44_100)
            var channels = sourceFormat.intOr(MediaFormat.KEY_CHANNEL_COUNT, 2)

            var muxTrack = -1
            var pcmBytesFed = 0L
            var extractorDone = false
            var decoderDone = false
            var encoderInputDone = false
            var encoderDone = false
            var pending: ByteBuffer? = null
            var stalled = 0
            val decoderInfo = MediaCodec.BufferInfo()
            val encoderInfo = MediaCodec.BufferInfo()

            while (!encoderDone) {
                var progressed = false

                // 1. Compressed input → decoder.
                if (!extractorDone) {
                    val inputIndex = decoder.dequeueInputBuffer(TIMEOUT_US)
                    if (inputIndex >= 0) {
                        progressed = true
                        val buffer = decoder.getInputBuffer(inputIndex)!!
                        val size = extractor.readSampleData(buffer, 0)
                        if (size < 0) {
                            decoder.queueInputBuffer(
                                inputIndex, 0, 0, 0,
                                MediaCodec.BUFFER_FLAG_END_OF_STREAM,
                            )
                            extractorDone = true
                        } else {
                            decoder.queueInputBuffer(
                                inputIndex, 0, size, extractor.sampleTime, 0,
                            )
                            extractor.advance()
                        }
                    }
                }

                // 2. PCM out of the decoder, one buffer held until fed onward.
                if (pending == null && !decoderDone) {
                    val outputIndex = decoder.dequeueOutputBuffer(decoderInfo, TIMEOUT_US)
                    when {
                        outputIndex == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                            progressed = true
                            val format = decoder.outputFormat
                            sampleRate = format.intOr(MediaFormat.KEY_SAMPLE_RATE, sampleRate)
                            channels = format.intOr(MediaFormat.KEY_CHANNEL_COUNT, channels)
                            if (encoder == null) {
                                encoder = createEncoder(sampleRate, channels)
                            }
                        }
                        outputIndex >= 0 -> {
                            progressed = true
                            if (decoderInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                                decoderDone = true
                            }
                            if (decoderInfo.size > 0) {
                                val buffer = decoder.getOutputBuffer(outputIndex)!!
                                buffer.position(decoderInfo.offset)
                                buffer.limit(decoderInfo.offset + decoderInfo.size)
                                // Copied out because the codec wants its buffer
                                // back before the encoder may be ready for it.
                                val copy = ByteBuffer.allocate(decoderInfo.size)
                                copy.put(buffer)
                                copy.flip()
                                pending = copy
                            }
                            decoder.releaseOutputBuffer(outputIndex, false)
                            // A decoder that never announced a format still has
                            // to meet an encoder before the first buffer.
                            if (encoder == null) {
                                encoder = createEncoder(sampleRate, channels)
                            }
                        }
                    }
                }

                // 3. PCM → encoder, timestamps derived from bytes fed so far.
                val activeEncoder = encoder
                if (activeEncoder != null && !encoderInputDone) {
                    val bytesPerSecond = sampleRate.toLong() * channels * 2
                    val fedUs = pcmBytesFed * 1_000_000 / bytesPerSecond
                    val chunk = pending
                    val finished = fedUs >= MAX_DURATION_US ||
                        (chunk == null && decoderDone)
                    if (finished) {
                        val inputIndex = activeEncoder.dequeueInputBuffer(TIMEOUT_US)
                        if (inputIndex >= 0) {
                            progressed = true
                            activeEncoder.queueInputBuffer(
                                inputIndex, 0, 0, fedUs,
                                MediaCodec.BUFFER_FLAG_END_OF_STREAM,
                            )
                            encoderInputDone = true
                            pending = null
                        }
                    } else if (chunk != null) {
                        val inputIndex = activeEncoder.dequeueInputBuffer(TIMEOUT_US)
                        if (inputIndex >= 0) {
                            progressed = true
                            val target = activeEncoder.getInputBuffer(inputIndex)!!
                            // The encoder's buffer can be smaller than a decoded
                            // frame, so the chunk is fed in as many pieces as
                            // it takes.
                            val size = minOf(target.remaining(), chunk.remaining())
                            val slice = chunk.duplicate()
                            slice.limit(slice.position() + size)
                            target.put(slice)
                            chunk.position(chunk.position() + size)
                            activeEncoder.queueInputBuffer(inputIndex, 0, size, fedUs, 0)
                            pcmBytesFed += size
                            if (!chunk.hasRemaining()) {
                                pending = null
                            }
                        }
                    }
                }

                // 4. AAC out of the encoder → muxer.
                if (activeEncoder != null) {
                    val outputIndex = activeEncoder.dequeueOutputBuffer(encoderInfo, TIMEOUT_US)
                    when {
                        outputIndex == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                            progressed = true
                            muxTrack = muxer.addTrack(activeEncoder.outputFormat)
                            muxer.start()
                            muxerStarted = true
                        }
                        outputIndex >= 0 -> {
                            progressed = true
                            if (encoderInfo.size > 0 &&
                                encoderInfo.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG == 0
                            ) {
                                check(muxerStarted) { "кодек выдал данные раньше формата" }
                                val buffer = activeEncoder.getOutputBuffer(outputIndex)!!
                                muxer.writeSampleData(muxTrack, buffer, encoderInfo)
                            }
                            if (encoderInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                                encoderDone = true
                            }
                            activeEncoder.releaseOutputBuffer(outputIndex, false)
                        }
                    }
                }

                // Every dequeue above has a timeout, so a broken codec would
                // otherwise spin here forever. A thousand empty passes at ten
                // milliseconds each is ten seconds of silence — enough to call
                // it dead.
                stalled = if (progressed) 0 else stalled + 1
                check(stalled < 1_000) { "кодек перестал отвечать" }
            }

            if (!muxerStarted) {
                throw IllegalArgumentException("аудиодорожка оказалась пустой")
            }
        } finally {
            runCatching { decoder?.stop() }
            runCatching { decoder?.release() }
            runCatching { encoder?.stop() }
            runCatching { encoder?.release() }
            if (muxerStarted) {
                runCatching { muxer?.stop() }
            }
            runCatching { muxer?.release() }
            runCatching { extractor.release() }
        }
    }

    private fun createEncoder(sampleRate: Int, channels: Int): MediaCodec {
        val format = MediaFormat.createAudioFormat(
            MediaFormat.MIMETYPE_AUDIO_AAC, sampleRate, channels,
        ).apply {
            setInteger(
                MediaFormat.KEY_AAC_PROFILE,
                android.media.MediaCodecInfo.CodecProfileLevel.AACObjectLC,
            )
            setInteger(MediaFormat.KEY_BIT_RATE, OUTPUT_BITRATE)
            // Decoded frames can be large; a roomy input buffer keeps the
            // feed-in-pieces loop short.
            setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, 64 * 1024)
        }
        return MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_AUDIO_AAC).apply {
            configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            start()
        }
    }

    private fun MediaFormat.intOr(key: String, fallback: Int): Int =
        if (containsKey(key)) getInteger(key) else fallback
}
