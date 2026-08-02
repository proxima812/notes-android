package dev.local.organizer.appicon

import android.app.Activity
import android.content.ComponentName
import android.content.pm.PackageManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
internal class SelectArgs {
    /** Alias name, without the package: `Ink`, `Amber`, and so on. */
    var alias: String = ""
    /** Every alias the build ships, so the others can be switched off. */
    var known: Array<String> = emptyArray()
}

/**
 * Switches which launcher icon the app shows.
 *
 * Android has no API for changing an icon; what it has is components that can
 * be turned on and off. Each icon is an `activity-alias` pointing at the one
 * real activity, and choosing an icon means enabling one alias and disabling
 * the rest.
 *
 * Which icons exist and which one is chosen is the core's business. This turns
 * components on and off and nothing else.
 */
@TauriPlugin
class AppIconPlugin(private val activity: Activity) : Plugin(activity) {

    @Command
    fun selectIcon(invoke: Invoke) {
        val args = invoke.parseArgs(SelectArgs::class.java)
        if (args.alias.isEmpty() || !args.known.contains(args.alias)) {
            invoke.reject("неизвестный вариант иконки")
            return
        }

        try {
            // Enabling first. Between the two calls the launcher may look at
            // the app, and a moment with no enabled alias is a moment with no
            // icon at all — on some launchers that sticks until a reboot.
            setEnabled(args.alias, enabled = true)
            for (other in args.known) {
                if (other != args.alias) {
                    setEnabled(other, enabled = false)
                }
            }
            invoke.resolve(JSObject().apply { put("alias", args.alias) })
        } catch (failure: Exception) {
            invoke.reject(failure.message ?: "не удалось сменить иконку")
        }
    }

    /** The alias currently showing, or an empty string if none is. */
    @Command
    fun currentIcon(invoke: Invoke) {
        val args = invoke.parseArgs(SelectArgs::class.java)
        val current = args.known.firstOrNull { alias ->
            activity.packageManager.getComponentEnabledSetting(component(alias)) ==
                PackageManager.COMPONENT_ENABLED_STATE_ENABLED
        }
        invoke.resolve(JSObject().apply { put("alias", current ?: "") })
    }

    private fun setEnabled(alias: String, enabled: Boolean) {
        val state = if (enabled) {
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED
        } else {
            PackageManager.COMPONENT_ENABLED_STATE_DISABLED
        }
        activity.packageManager.setComponentEnabledSetting(
            component(alias),
            state,
            // Killing the app would take the settings screen down with it,
            // right as the user taps an icon.
            PackageManager.DONT_KILL_APP,
        )
    }

    private fun component(alias: String) =
        ComponentName(activity, "dev.local.organizer.appicon.$alias")
}
