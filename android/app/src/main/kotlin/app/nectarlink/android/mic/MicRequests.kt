// SPDX-License-Identifier: GPL-3.0-or-later
package app.nectarlink.android.mic

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import app.nectarlink.android.R

/**
 * A PC asking for this phone's microphone (`mic.start`). The microphone only turns on when
 * the user taps: the notification opens [MicStartActivity], which asks for the microphone
 * permission if needed and starts [MicService] from the foreground, as Android requires.
 */
object MicRequests {
    private const val CHANNEL = "mic_requests"
    private const val TAG = "mic_req"
    private const val REQUEST_TIMEOUT_MS = 2 * 60 * 1000L

    /** Shows the request; false if notifications are off. */
    fun show(context: Context, pcId: String, pcName: String): Boolean {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            return false
        }
        context.getSystemService(NotificationManager::class.java)?.createNotificationChannel(
            NotificationChannel(CHANNEL, context.getString(R.string.channel_mic_requests), NotificationManager.IMPORTANCE_HIGH),
        )
        val start = PendingIntent.getActivity(
            context,
            pcId.hashCode(),
            Intent(context, MicStartActivity::class.java)
                .putExtra(MicStartActivity.EXTRA_PC, pcId)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val from = pcName.ifEmpty { context.getString(R.string.your_pc) }
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(context.getString(R.string.mic_request_title, from))
            .setContentText(context.getString(R.string.mic_request_text))
            .setContentIntent(start)
            .addAction(0, context.getString(R.string.mic_request_start), start)
            .setAutoCancel(true)
            .setTimeoutAfter(REQUEST_TIMEOUT_MS)
            .setCategory(NotificationCompat.CATEGORY_CALL)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .build()
        return runCatching { NotificationManagerCompat.from(context).notify(TAG, pcId.hashCode(), notification) }.isSuccess
    }

    fun dismiss(context: Context, pcId: String) {
        runCatching { NotificationManagerCompat.from(context).cancel(TAG, pcId.hashCode()) }
    }
}

/** Starts the microphone for a PC after the user tapped its request; shows nothing itself. */
class MicStartActivity : ComponentActivity() {
    private var pcId: String? = null

    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) pcId?.let { MicService.start(this, it) }
        finish()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val pc = intent.getStringExtra(EXTRA_PC)
        if (pc == null) {
            finish()
            return
        }
        pcId = pc
        MicRequests.dismiss(this, pc)
        if (MicService.hasPermission(this)) {
            MicService.start(this, pc)
            finish()
        } else if (savedInstanceState == null) {
            permission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    companion object {
        const val EXTRA_PC = "mic_pc"
    }
}
