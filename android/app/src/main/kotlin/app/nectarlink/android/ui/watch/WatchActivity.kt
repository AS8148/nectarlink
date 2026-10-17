// SPDX-License-Identifier: GPL-3.0-or-later
package app.nectarlink.android.ui.watch

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.nectarlink.android.NectarlinkApplication
import app.nectarlink.android.R
import app.nectarlink.android.core.Core
import app.nectarlink.android.media.WatchOffers
import app.nectarlink.android.ui.Preferences
import app.nectarlink.android.ui.remote.PcMirrorView
import app.nectarlink.android.ui.theme.LocalReducedMotion
import app.nectarlink.android.ui.theme.NectarlinkTheme
import app.nectarlink.core.MediaAction
import kotlinx.coroutines.delay

/**
 * Play on phone: what a PC player plays, streamed from the PC with its sound, full screen.
 * A tap shows what it is, play and pause (on the PC's player) and close; they fade after a
 * few seconds. Nothing else goes to the PC.
 */
class WatchActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val pcId = intent.getStringExtra(EXTRA_PC)
        val player = intent.getStringExtra(EXTRA_PLAYER)
        if (pcId == null || player == null) {
            finish()
            return
        }
        val pcName = intent.getStringExtra(EXTRA_PC_NAME).orEmpty()
        val title = intent.getStringExtra(EXTRA_TITLE)
        WatchOffers.dismiss(this, pcId)
        val core = (application as NectarlinkApplication).core
        val preferences = Preferences(this)
        setContent {
            val appearance by preferences.appearance.collectAsStateWithLifecycle()
            NectarlinkTheme(appearance) {
                WatchScreen(core, pcId, pcName, player, title, onClose = ::finish)
            }
        }
    }

    companion object {
        private const val EXTRA_PC = "watch_pc"
        private const val EXTRA_PC_NAME = "watch_pc_name"
        private const val EXTRA_PLAYER = "watch_player"
        private const val EXTRA_TITLE = "watch_title"

        fun intent(context: Context, pcId: String, pcName: String, player: String, title: String?): Intent =
            Intent(context, WatchActivity::class.java)
                .putExtra(EXTRA_PC, pcId)
                .putExtra(EXTRA_PC_NAME, pcName)
                .putExtra(EXTRA_PLAYER, player)
                .putExtra(EXTRA_TITLE, title)
    }
}

@Composable
private fun WatchScreen(
    core: Core,
    pcId: String,
    pcName: String,
    player: String,
    title: String?,
    onClose: () -> Unit,
) {
    val reducedMotion = LocalReducedMotion.current
    val playing by core.pcMedia.playing.collectAsStateWithLifecycle()
    val isPlaying = playing["$pcId/$player"] ?: true
    var controls by remember { mutableStateOf(true) }
    var shownAt by remember { mutableIntStateOf(0) }

    // The controls fade a few seconds after they show or are used.
    LaunchedEffect(controls, shownAt) {
        if (controls) {
            delay(CONTROLS_MS)
            controls = false
        }
    }

    Box(Modifier.fillMaxSize().background(Color.Black)) {
        PcMirrorView(
            pcId = pcId,
            pcName = pcName,
            core = core,
            modifier = Modifier.fillMaxSize(),
            isFullScreen = true,
            onFullScreenChange = { full -> if (!full) onClose() },
            watch = player,
            onTap = {
                controls = !controls
                shownAt++
            },
        )
        AnimatedVisibility(
            visible = controls,
            enter = if (reducedMotion) EnterTransition.None else fadeIn(tween(180)),
            exit = if (reducedMotion) ExitTransition.None else fadeOut(tween(220)),
            modifier = Modifier.fillMaxSize(),
        ) {
            Column(Modifier.fillMaxSize()) {
                // What it is and where from, over a shade so it reads on any picture.
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(Brush.verticalGradient(listOf(Color.Black.copy(alpha = 0.7f), Color.Transparent)))
                        .padding(horizontal = 20.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            text = title?.takeIf { it.isNotBlank() } ?: stringResource(R.string.watch_offer_untitled),
                            style = MaterialTheme.typography.titleMedium,
                            color = Color.White,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            text = stringResource(R.string.watch_from, pcName),
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.White.copy(alpha = 0.75f),
                            maxLines = 1,
                        )
                    }
                    val closeLabel = stringResource(R.string.watch_close)
                    IconButton(
                        onClick = onClose,
                        colors = IconButtonDefaults.iconButtonColors(
                            containerColor = Color.White.copy(alpha = 0.14f),
                            contentColor = Color.White,
                        ),
                        modifier = Modifier.size(40.dp).semantics { contentDescription = closeLabel },
                    ) {
                        CloseIcon()
                    }
                }
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    val label = stringResource(if (isPlaying) R.string.media_pause else R.string.media_play)
                    IconButton(
                        onClick = {
                            core.pcMedia.command(pcId, player, if (isPlaying) MediaAction.PAUSE else MediaAction.PLAY)
                            shownAt++
                        },
                        colors = IconButtonDefaults.iconButtonColors(
                            containerColor = Color.Black.copy(alpha = 0.5f),
                            contentColor = Color.White,
                        ),
                        modifier = Modifier.size(64.dp).semantics { contentDescription = label },
                    ) {
                        Icon(
                            painter = painterResource(if (isPlaying) R.drawable.ic_pause else R.drawable.ic_play),
                            contentDescription = null,
                            modifier = Modifier.size(30.dp),
                        )
                    }
                }
            }
        }
    }
}

/** An X, in the stroke style of the app's other drawn icons. */
@Composable
private fun CloseIcon() {
    Canvas(Modifier.size(16.dp)) {
        val width = 2.dp.toPx()
        drawLine(Color.White, Offset(0f, 0f), Offset(size.width, size.height), width, StrokeCap.Round)
        drawLine(Color.White, Offset(size.width, 0f), Offset(0f, size.height), width, StrokeCap.Round)
    }
}

/** How long the controls stay after they show or are used. */
private const val CONTROLS_MS = 3_000L
