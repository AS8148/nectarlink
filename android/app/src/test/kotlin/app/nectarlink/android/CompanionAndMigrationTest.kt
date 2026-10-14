// SPDX-License-Identifier: GPL-3.0-or-later
package app.nectarlink.android

import app.nectarlink.android.clipboard.SendActivity
import app.nectarlink.android.companion.PcCompanion
import app.nectarlink.android.ui.Preferences
import app.nectarlink.android.ui.theme.Appearance
import app.nectarlink.android.ui.theme.Tokens
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CompanionAndMigrationTest {

    @Test
    fun `PcCompanion buildBeaconPayload and buildBeaconMask encode 5-byte magic and 8-byte device ID prefix`() {
        val hexId = "0123456789abcdef-fedcba9876543210"
        val payload = PcCompanion.buildBeaconPayload(hexId)
        assertEquals(13, payload.size)
        assertArrayEquals(
            byteArrayOf(
                'N'.code.toByte(),
                'C'.code.toByte(),
                'T'.code.toByte(),
                'R'.code.toByte(),
                1,
                0x01,
                0x23,
                0x45,
                0x67,
                0x89.toByte(),
                0xAB.toByte(),
                0xCD.toByte(),
                0xEF.toByte(),
            ),
            payload,
        )

        val maskWithId = PcCompanion.buildBeaconMask(hasDeviceId = true)
        assertEquals(13, maskWithId.size)
        assertTrue(maskWithId.all { it == 0xFF.toByte() })

        val shortPayload = PcCompanion.buildBeaconPayload("abcd")
        assertArrayEquals(PcCompanion.BEACON_MAGIC, shortPayload)
        assertArrayEquals(PcCompanion.BEACON_MAGIC, PcCompanion.buildBeaconPayload(null))

        val maskWithoutId = PcCompanion.buildBeaconMask(hasDeviceId = false)
        assertEquals(5, maskWithoutId.size)
        assertTrue(maskWithoutId.all { it == 0xFF.toByte() })
    }

    @Test
    fun `Preferences sanitizeAppearance falls back on unknown legacy theme, mode, and seed values`() {
        val legacy = Preferences.sanitizeAppearance(
            theme = "sepia",
            mode = "neon",
            seed = "cyberpunk",
            dynamicColor = false,
        )
        assertEquals("bloom", legacy.theme)
        assertEquals("system", legacy.mode)
        assertEquals(Tokens.DEFAULT_SEED, legacy.seed)
        assertFalse(legacy.dynamicColor)

        val empty = Preferences.sanitizeAppearance(
            Appearance(theme = "", mode = "", seed = "", dynamicColor = true),
        )
        assertEquals("bloom", empty.theme)
        assertEquals("system", empty.mode)
        assertEquals(Tokens.DEFAULT_SEED, empty.seed)
        assertTrue(empty.dynamicColor)

        val valid = Preferences.sanitizeAppearance(
            theme = "graphite",
            mode = "dark",
            seed = "ocean",
            dynamicColor = false,
        )
        assertEquals("graphite", valid.theme)
        assertEquals("dark", valid.mode)
        assertEquals("ocean", valid.seed)
        assertFalse(valid.dynamicColor)
    }

    @Test
    fun `SendActivity shouldTriggerFlickSend triggers on fast upward flick or deep upward drag`() {
        // Fast upward flick (< -1000 px/s) triggers even with minimal displacement
        assertTrue(
            SendActivity.shouldTriggerFlickSend(
                velocityYPxPerSec = -1250f,
                dragOffsetYPx = -24f,
                density = 2f,
            ),
        )
        // Deep upward drag (> 110 dp = 220 px at density 2.0) triggers even with slow release velocity
        assertTrue(
            SendActivity.shouldTriggerFlickSend(
                velocityYPxPerSec = -150f,
                dragOffsetYPx = -230f,
                density = 2f,
            ),
        )
        // Small drag and slow velocity does not trigger send
        assertFalse(
            SendActivity.shouldTriggerFlickSend(
                velocityYPxPerSec = -400f,
                dragOffsetYPx = -120f,
                density = 2f,
            ),
        )
        // Downward flick never triggers send
        assertFalse(
            SendActivity.shouldTriggerFlickSend(
                velocityYPxPerSec = 1800f,
                dragOffsetYPx = 30f,
                density = 2f,
            ),
        )
    }
}
