// SPDX-License-Identifier: GPL-3.0-or-later
package app.nectarlink.android.media

import org.junit.Assert.assertEquals
import org.junit.Test

class PcVolumeStepsTest {
    @Test
    fun pcVolumeRoundsToTheNearestStep() {
        assertEquals(0, PcMedia.toSteps(0))
        assertEquals(0, PcMedia.toSteps(1))
        assertEquals(1, PcMedia.toSteps(2))
        assertEquals(9, PcMedia.toSteps(37))
        assertEquals(PcMedia.VOLUME_STEPS, PcMedia.toSteps(100))
    }

    @Test
    fun outOfRangeVolumesAreClamped() {
        assertEquals(0, PcMedia.toSteps(-5))
        assertEquals(PcMedia.VOLUME_STEPS, PcMedia.toSteps(140))
    }

    @Test
    fun everyStepMapsBackToItself() {
        for (step in 0..PcMedia.VOLUME_STEPS) {
            assertEquals(step, PcMedia.toSteps(step * 100 / PcMedia.VOLUME_STEPS))
        }
    }
}
