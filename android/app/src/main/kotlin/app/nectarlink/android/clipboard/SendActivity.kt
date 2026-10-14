// SPDX-License-Identifier: GPL-3.0-or-later
package app.nectarlink.android.clipboard

import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.core.content.IntentCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.nectarlink.android.NectarlinkApplication
import app.nectarlink.android.R
import app.nectarlink.android.core.Device
import app.nectarlink.android.ui.Preferences
import app.nectarlink.android.ui.theme.NectarlinkTheme
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Sends to the connected PCs: files or text shared from another app, or the
 * clipboard (from the Quick Settings tile or the connection notification).
 * Direct Share targets and clipboard sends dispatch immediately; choosing the
 * main Nectarlink target in the Android share sheet opens the interactive
 * [FlickSendSheet] with upward flick-to-send and a 3-second Undo window.
 */
class SendActivity : ComponentActivity() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var sent = false
    private val pickFilesLauncher = registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.OpenMultipleDocuments(),
    ) { uris ->
        if (uris.isNullOrEmpty()) {
            finish()
        } else {
            sendFiles(uris, handoff = false)
        }
    }

    private sealed interface SharePayload {
        val previewTitle: String
        val previewSubtitle: String?

        data class Files(
            val uris: List<Uri>,
            val handoff: Boolean,
            override val previewTitle: String,
            override val previewSubtitle: String?,
        ) : SharePayload

        data class Link(
            val url: String,
            val text: String,
            override val previewTitle: String = url,
            override val previewSubtitle: String? = text.takeIf { it != url },
        ) : SharePayload

        data class Text(
            val text: String,
            val html: String?,
            override val previewTitle: String = text,
            override val previewSubtitle: String? = null,
        ) : SharePayload
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val intent = intent ?: return finish()
        if (intent.action == ACTION_SEND_CLIPBOARD) {
            // The clipboard is read in onWindowFocusChanged.
            return
        }
        if (intent.action == ACTION_SEND_FILES) {
            if (savedInstanceState == null) {
                pickFilesLauncher.launch(arrayOf("*/*"))
            }
            return
        }
        val payload = parseSharePayload(intent)
        if (payload == null) {
            val msg = if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
                getString(R.string.transfer_nothing)
            } else {
                getString(R.string.clip_nothing_shared)
            }
            finishWith(msg)
            return
        }

        val core = (application as NectarlinkApplication).core
        val allPcs = core.state.value.devices.sortedByDescending { it.online }
        val immediate = intent.getBooleanExtra("immediate", false) ||
            targetPcId() != null ||
            (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && intent.getStringExtra(Intent.EXTRA_SHORTCUT_ID) != null)

        if (immediate || allPcs.isEmpty()) {
            dispatchImmediate(payload)
            return
        }

        val preferences = Preferences(this)
        setContent {
            val appearance by preferences.appearance.collectAsStateWithLifecycle()
            val state by core.state.collectAsStateWithLifecycle()
            val pcs = state.devices.sortedByDescending { it.online }.ifEmpty { allPcs }
            NectarlinkTheme(appearance) {
                FlickSendSheet(
                    pcs = pcs,
                    payload = payload,
                    onCommitSend = { pcId ->
                        dispatchToPc(payload, pcId)
                    },
                    onDismiss = { finish() },
                )
            }
        }
    }

    private fun parseSharePayload(intent: Intent): SharePayload? = when (intent.action) {
        Intent.ACTION_VIEW -> {
            val uri = intent.data
            if (uri != null && uri.scheme?.lowercase() == "geo") {
                val geo = uri.toString()
                SharePayload.Link(url = geo, text = geo)
            } else {
                null
            }
        }
        Intent.ACTION_SEND -> {
            val stream = IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
            val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
            val html = intent.getStringExtra(Intent.EXTRA_HTML_TEXT)?.takeIf(String::isNotBlank)
            val link = text?.let(::handoffUrl)
            when {
                stream != null -> SharePayload.Files(
                    uris = listOf(stream),
                    handoff = true,
                    previewTitle = stream.lastPathSegment ?: getString(R.string.flick_send_single_file),
                    previewSubtitle = intent.type,
                )
                !text.isNullOrBlank() && link != null -> SharePayload.Link(url = link, text = text)
                !text.isNullOrBlank() -> SharePayload.Text(text = text, html = html)
                else -> null
            }
        }
        Intent.ACTION_SEND_MULTIPLE -> {
            val streams = IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
            if (streams.isNullOrEmpty()) {
                null
            } else {
                SharePayload.Files(
                    uris = streams,
                    handoff = false,
                    previewTitle = if (streams.size == 1) {
                        getString(R.string.flick_send_single_file)
                    } else {
                        getString(R.string.flick_send_files_count, streams.size)
                    },
                    previewSubtitle = intent.type,
                )
            }
        }
        else -> null
    }

    private fun dispatchImmediate(payload: SharePayload) {
        when (payload) {
            is SharePayload.Files -> sendFiles(payload.uris, payload.handoff)
            is SharePayload.Link -> sendLink(payload.url, payload.text)
            is SharePayload.Text -> send(payload.text, payload.html)
        }
    }

    private fun dispatchToPc(payload: SharePayload, pcId: String) {
        if (sent) return
        sent = true
        val core = (application as NectarlinkApplication).core
        val pc = core.state.value.device(pcId)
        when (payload) {
            is SharePayload.Files -> {
                core.sendFiles(pcId, payload.uris, payload.handoff)
                val name = pc?.name ?: pcId
                val msg = if (pc != null && !pc.online) {
                    getString(R.string.send_queue_offline_toast, name)
                } else {
                    getString(R.string.transfer_sending, name)
                }
                finishWith(msg)
            }
            is SharePayload.Link -> {
                if (pc != null && (pc.has("device.links_to_pc") || !pc.online)) {
                    scope.launch { finishWith(core.openLinkOnPc(pcId, payload.url)) }
                } else {
                    scope.launch { finishWith(core.sendClipboard(payload.text, pcId)) }
                }
            }
            is SharePayload.Text -> {
                scope.launch { finishWith(core.sendClipboard(payload.text, pcId, payload.html)) }
            }
        }
    }

    private fun targetPcId(): String? {
        val explicit = intent?.getStringExtra(EXTRA_PC_ID)?.takeIf { it.isNotBlank() }
        if (explicit != null) return explicit
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val shortcutId = intent?.getStringExtra(Intent.EXTRA_SHORTCUT_ID)
            DirectShareTargets.pcIdFromShortcutId(shortcutId)?.let { return it }
        }
        return null
    }

    /** Files go to one PC: the connected one, or the one the user picks (queueing if offline). */
    private fun sendFiles(uris: List<Uri>, handoff: Boolean = false) {
        sent = true
        val core = (application as NectarlinkApplication).core
        val chosenPcId = targetPcId()
        if (chosenPcId != null) {
            val pc = core.state.value.device(chosenPcId)
            if (pc != null) {
                core.sendFiles(pc.id, uris, handoff)
                val msg = if (pc.online) {
                    getString(R.string.transfer_sending, pc.name)
                } else {
                    getString(R.string.send_queue_offline_toast, pc.name)
                }
                finishWith(msg)
                return
            }
        }
        val allPcs = core.state.value.devices
        val pcs = allPcs.filter { it.online }.ifEmpty { allPcs }
        when (pcs.size) {
            0 -> finishWith(getString(R.string.clip_no_pc))
            1 -> {
                val target = pcs[0]
                core.sendFiles(target.id, uris, handoff)
                val msg = if (target.online) {
                    getString(R.string.transfer_sending, target.name)
                } else {
                    getString(R.string.send_queue_offline_toast, target.name)
                }
                finishWith(msg)
            }
            else -> AlertDialog.Builder(this)
                .setTitle(R.string.transfer_pick_pc)
                .setItems(pcs.map { it.name }.toTypedArray()) { _, which ->
                    val target = pcs[which]
                    core.sendFiles(target.id, uris, handoff)
                    val msg = if (target.online) {
                        getString(R.string.transfer_sending, target.name)
                    } else {
                        getString(R.string.send_queue_offline_toast, target.name)
                    }
                    finishWith(msg)
                }
                .setOnCancelListener { finish() }
                .show()
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || sent || intent?.action != ACTION_SEND_CLIPBOARD) return
        when (val clip = PhoneClipboard.read(this)) {
            is PhoneClipboard.Read.Text -> send(clip.text, clip.html)
            is PhoneClipboard.Read.Image -> sendImage(clip.uri)
            PhoneClipboard.Read.Private -> finishWith(getString(R.string.clip_private))
            PhoneClipboard.Read.Empty -> finishWith(getString(R.string.clip_empty))
        }
    }

    /**
     * A shared link, video with timestamp, or map location opens on the PC
     * (the one that takes links; the user picks when there are several).
     * With no such PC, it goes to the clipboard like other text.
     */
    private fun sendLink(url: String, text: String) {
        sent = true
        val core = (application as NectarlinkApplication).core
        val chosenPcId = targetPcId()
        if (chosenPcId != null) {
            val pc = core.state.value.device(chosenPcId)
            if (pc != null) {
                if (!pc.online || pc.has("device.links_to_pc")) {
                    scope.launch { finishWith(core.openLinkOnPc(pc.id, url)) }
                } else {
                    send(text)
                }
                return
            }
        }
        val onlineLinkPcs = core.state.value.devices.filter { it.online && it.has("device.links_to_pc") }
        val pcs = onlineLinkPcs.ifEmpty {
            if (core.state.value.devices.none { it.online }) core.state.value.devices else emptyList()
        }
        when (pcs.size) {
            0 -> send(text)
            1 -> scope.launch { finishWith(core.openLinkOnPc(pcs[0].id, url)) }
            else -> AlertDialog.Builder(this)
                .setTitle(R.string.link_pick_pc)
                .setItems(pcs.map { it.name }.toTypedArray()) { _, which ->
                    scope.launch { finishWith(core.openLinkOnPc(pcs[which].id, url)) }
                }
                .setOnCancelListener { finish() }
                .show()
        }
    }

    private fun send(text: String, html: String? = null) {
        sent = true
        val core = (application as NectarlinkApplication).core
        val pcId = targetPcId()
        scope.launch { finishWith(core.sendClipboard(text, pcId, html)) }
    }

    private fun sendImage(uri: Uri) {
        sent = true
        val core = (application as NectarlinkApplication).core
        val pcId = targetPcId()
        scope.launch { finishWith(core.sendClipboardImage(uri, pcId)) }
    }

    private fun finishWith(message: String) {
        sent = true
        Toast.makeText(applicationContext, message, Toast.LENGTH_SHORT).show()
        finish()
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    @OptIn(ExperimentalLayoutApi::class)
    @Composable
    private fun FlickSendSheet(
        pcs: List<Device>,
        payload: SharePayload,
        onCommitSend: (String) -> Unit,
        onDismiss: () -> Unit,
    ) {
        var selectedPcId by remember(pcs) { mutableStateOf(pcs.firstOrNull()?.id.orEmpty()) }
        val selectedPc = pcs.firstOrNull { it.id == selectedPcId } ?: pcs.firstOrNull()
        val pcName = selectedPc?.name ?: "PC"

        var countdownActive by remember { mutableStateOf(false) }
        var countdownProgress by remember { mutableFloatStateOf(1f) }
        val dragOffset = remember { Animatable(0f) }
        val uiScope = rememberCoroutineScope()
        val density = LocalDensity.current.density

        LaunchedEffect(countdownActive, selectedPc?.id) {
            val targetId = selectedPc?.id ?: return@LaunchedEffect
            if (!countdownActive) return@LaunchedEffect
            val durationMs = 3_000L
            val stepMs = 50L
            var elapsed = 0L
            while (elapsed < durationMs && countdownActive) {
                countdownProgress = 1f - (elapsed.toFloat() / durationMs.toFloat())
                delay(stepMs)
                elapsed += stepMs
            }
            if (countdownActive) {
                onCommitSend(targetId)
            }
        }

        val triggerSendWithUndo = {
            uiScope.launch {
                dragOffset.animateTo(
                    targetValue = -1200f,
                    animationSpec = spring(
                        dampingRatio = Spring.DampingRatioLowBouncy,
                        stiffness = Spring.StiffnessMediumLow,
                    ),
                )
                countdownProgress = 1f
                countdownActive = true
            }
        }

        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Black.copy(alpha = 0.45f))
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = { if (!countdownActive) onDismiss() },
                ),
            contentAlignment = Alignment.BottomCenter,
        ) {
            Surface(
                shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 6.dp,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                        onClick = {},
                    ),
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .navigationBarsPadding()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Text(
                        text = stringResource(R.string.flick_send_title, pcName),
                        style = MaterialTheme.typography.titleLarge,
                    )

                    if (pcs.size > 1) {
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            pcs.forEach { pc ->
                                FilterChip(
                                    selected = pc.id == selectedPc?.id,
                                    onClick = { selectedPcId = pc.id },
                                    label = { Text(pc.name) },
                                )
                            }
                        }
                    }

                    if (!countdownActive) {
                        Text(
                            text = stringResource(R.string.flick_send_hint, pcName),
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )

                        ElevatedCard(
                            colors = CardDefaults.elevatedCardColors(
                                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                            ),
                            modifier = Modifier
                                .fillMaxWidth()
                                .offset { IntOffset(0, dragOffset.value.roundToInt()) }
                                .pointerInput(selectedPc?.id) {
                                    val velocityTracker = VelocityTracker()
                                    detectVerticalDragGestures(
                                        onDragStart = { velocityTracker.resetTracking() },
                                        onVerticalDrag = { change, dragAmount ->
                                            change.consume()
                                            velocityTracker.addPosition(change.uptimeMillis, change.position)
                                            val next = (dragOffset.value + dragAmount).coerceAtMost(36f)
                                            uiScope.launch { dragOffset.snapTo(next) }
                                        },
                                        onDragCancel = {
                                            uiScope.launch {
                                                dragOffset.animateTo(0f, spring(stiffness = Spring.StiffnessMedium))
                                            }
                                        },
                                        onDragEnd = {
                                            val velocityY = velocityTracker.calculateVelocity().y
                                            if (shouldTriggerFlickSend(velocityY, dragOffset.value, density)) {
                                                triggerSendWithUndo()
                                            } else {
                                                uiScope.launch {
                                                    dragOffset.animateTo(0f, spring(stiffness = Spring.StiffnessMedium))
                                                }
                                            }
                                        },
                                    )
                                },
                        ) {
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(16.dp),
                                verticalArrangement = Arrangement.spacedBy(6.dp),
                            ) {
                                Text(
                                    text = payload.previewTitle,
                                    style = MaterialTheme.typography.bodyLarge,
                                    maxLines = 4,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                payload.previewSubtitle?.takeIf { it.isNotBlank() }?.let { sub ->
                                    Text(
                                        text = sub,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            }
                        }

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.End),
                        ) {
                            TextButton(onClick = onDismiss) {
                                Text(stringResource(R.string.action_cancel))
                            }
                            Button(onClick = { triggerSendWithUndo() }) {
                                Text(stringResource(R.string.flick_send_now))
                            }
                        }
                    } else {
                        Column(
                            modifier = Modifier.fillMaxWidth(),
                            verticalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Text(
                                    text = stringResource(R.string.flick_send_sending, pcName),
                                    style = MaterialTheme.typography.titleMedium,
                                )
                                OutlinedButton(
                                    onClick = {
                                        countdownActive = false
                                        uiScope.launch {
                                            dragOffset.animateTo(0f, spring(stiffness = Spring.StiffnessMedium))
                                        }
                                    },
                                ) {
                                    Text(stringResource(R.string.flick_send_undo))
                                }
                            }
                            LinearProgressIndicator(
                                progress = { countdownProgress },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Spacer(Modifier.height(4.dp))
                        }
                    }
                }
            }
        }
    }

    companion object {
        const val FLICK_VELOCITY_THRESHOLD_PX_PER_SEC: Float = -1000f
        const val FLICK_DISPLACEMENT_THRESHOLD_DP: Float = 110f

        /**
         * Returns true when an upward flick gesture exceeds either the velocity
         * threshold (`< -1000 px/s`) or the upward displacement threshold (`> 110 dp`).
         */
        fun shouldTriggerFlickSend(
            velocityYPxPerSec: Float,
            dragOffsetYPx: Float,
            density: Float = 1f,
        ): Boolean {
            val minDragPx = -FLICK_DISPLACEMENT_THRESHOLD_DP * density
            return velocityYPxPerSec < FLICK_VELOCITY_THRESHOLD_PX_PER_SEC || dragOffsetYPx < minDragPx
        }

        /** The text as a web link, if it's one (apps often share "Title https://…"). */
        fun webLink(text: String): String? {
            val words = text.trim().split(Regex("\\s+"))
            val link = words.lastOrNull() ?: return null
            val lower = link.lowercase()
            val web = (lower.startsWith("https://") || lower.startsWith("http://")) && link.length in 9..4096
            // Only when the link is the point: alone, or after a short title.
            return link.takeIf { web && words.size <= 12 }
        }

        /**
         * Extracts a Handoff URL (web link, YouTube/video URL with timestamp,
         * `geo:` map URI, or street address converted to `geo:`) from shared text.
         */
        fun handoffUrl(text: String): String? {
            app.nectarlink.core.extractHandoffLink(text)?.let { return it.url }
            val suggestion = app.nectarlink.core.classifyClip(text)
            if (suggestion?.kind == app.nectarlink.core.ClipKind.STREET_ADDRESS) {
                return "geo:0,0?q=${Uri.encode(text.trim())}"
            }
            return webLink(text)
        }

        const val ACTION_SEND_CLIPBOARD = "app.nectarlink.action.SEND_CLIPBOARD"
        const val ACTION_SEND_FILES = "app.nectarlink.action.SEND_FILES"
        const val EXTRA_PC_ID = "app.nectarlink.extra.PC_ID"

        fun sendClipboardIntent(context: Context, pcId: String? = null): Intent =
            Intent(context, SendActivity::class.java)
                .setAction(ACTION_SEND_CLIPBOARD)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_ANIMATION)
                .apply {
                    if (!pcId.isNullOrEmpty()) putExtra(EXTRA_PC_ID, pcId)
                }

        fun sendFilesIntent(context: Context, pcId: String? = null): Intent =
            Intent(context, SendActivity::class.java)
                .setAction(ACTION_SEND_FILES)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_ANIMATION)
                .apply {
                    if (!pcId.isNullOrEmpty()) putExtra(EXTRA_PC_ID, pcId)
                }
    }
}
