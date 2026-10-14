// SPDX-License-Identifier: GPL-3.0-or-later
package app.nectarlink.android.notifications

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.BitmapFactory
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import app.nectarlink.android.R
import app.nectarlink.android.ui.MainActivity
import java.util.concurrent.ConcurrentHashMap

typealias MirroredNotification = app.nectarlink.core.Notification

/**
 * Posts notifications mirrored from a paired PC (`notify.push` / `notify.list` / `notify.dismiss`)
 * into Android's system notification shade.
 */
class PcNotificationPoster(context: Context) {
    private val context = context.applicationContext
    private val manager: NotificationManager? =
        this.context.getSystemService(NotificationManager::class.java)
    private val activeKeysByDevice = ConcurrentHashMap<String, MutableSet<String>>()

    init {
        val channel = NotificationChannel(
            CHANNEL_ID,
            this.context.getString(R.string.channel_pc_notifications),
            NotificationManager.IMPORTANCE_DEFAULT,
        )
        manager?.createNotificationChannel(channel)
    }

    fun post(deviceId: String, n: MirroredNotification) {
        val nm = manager ?: return
        if (!canPostNotifications()) return
        val keys = activeKeysByDevice.getOrPut(deviceId) { ConcurrentHashMap.newKeySet() }
        keys.add(n.key)

        val tapIntent = PendingIntent.getActivity(
            context,
            notificationId(deviceId, n.key),
            Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val title = n.title?.takeIf(String::isNotBlank) ?: n.appName.ifBlank { context.getString(R.string.your_pc) }
        val body = n.text.orEmpty()
        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setSubText(n.appName.takeIf(String::isNotBlank))
            .setAutoCancel(true)
            .setSilent(n.silent)
            .setContentIntent(tapIntent)

        if (body.length > 48) {
            builder.setStyle(NotificationCompat.BigTextStyle().bigText(body))
        }
        if (n.`when` > 0L) {
            builder.setWhen(n.`when`).setShowWhen(true)
        }
        n.icon?.let { iconBytes ->
            runCatching { BitmapFactory.decodeByteArray(iconBytes, 0, iconBytes.size) }
                .getOrNull()
                ?.let(builder::setLargeIcon)
        }

        nm.notify(notificationId(deviceId, n.key), builder.build())
    }

    fun cancel(deviceId: String, key: String) {
        activeKeysByDevice[deviceId]?.remove(key)
        manager?.cancel(notificationId(deviceId, key))
    }

    fun reset(deviceId: String, items: List<MirroredNotification>) {
        val newKeys = items.map { it.key }.toSet()
        val previous = activeKeysByDevice[deviceId]?.toSet().orEmpty()
        for (removedKey in previous - newKeys) {
            manager?.cancel(notificationId(deviceId, removedKey))
        }
        for (item in items) {
            post(deviceId, item)
        }
    }

    fun cancelAll(deviceId: String) {
        val keys = activeKeysByDevice.remove(deviceId) ?: return
        for (key in keys) {
            manager?.cancel(notificationId(deviceId, key))
        }
    }

    private fun notificationId(deviceId: String, key: String): Int =
        (deviceId + ":" + key).hashCode()

    private fun canPostNotifications(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return true
        return ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.POST_NOTIFICATIONS,
        ) == PackageManager.PERMISSION_GRANTED
    }

    companion object {
        const val CHANNEL_ID = "pc_notifications"
    }
}
