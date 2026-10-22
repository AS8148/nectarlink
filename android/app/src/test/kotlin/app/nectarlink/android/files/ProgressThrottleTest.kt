// SPDX-License-Identifier: GPL-3.0-or-later
package app.nectarlink.android.files

import app.nectarlink.android.files.TransferNotifications.ProgressThrottle
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ProgressThrottleTest {
    @Test
    fun postsTheFirstUpdateRightAway() {
        assertTrue(ProgressThrottle().shouldPost("t", 0, nowMs = 0))
    }

    @Test
    fun postsAChangedPercentAtMostOnceASecond() {
        val t = ProgressThrottle()
        assertTrue(t.shouldPost("t", 1, nowMs = 0))
        assertFalse(t.shouldPost("t", 2, nowMs = 250))
        assertFalse(t.shouldPost("t", 3, nowMs = 999))
        assertTrue(t.shouldPost("t", 4, nowMs = 1_000))
    }

    @Test
    fun repostsAnUnchangedPercentOnlyEveryFewSeconds() {
        val t = ProgressThrottle()
        assertTrue(t.shouldPost("t", 7, nowMs = 0))
        assertFalse(t.shouldPost("t", 7, nowMs = 1_500))
        assertFalse(t.shouldPost("t", 7, nowMs = 4_999))
        assertTrue(t.shouldPost("t", 7, nowMs = 5_000))
    }

    @Test
    fun keepsTransfersApartAndStartsOverAfterForget() {
        val t = ProgressThrottle()
        assertTrue(t.shouldPost("a", 1, nowMs = 0))
        assertTrue(t.shouldPost("b", 1, nowMs = 100))
        assertFalse(t.shouldPost("a", 2, nowMs = 200))
        t.forget("a")
        assertTrue(t.shouldPost("a", 2, nowMs = 300))
    }
}
