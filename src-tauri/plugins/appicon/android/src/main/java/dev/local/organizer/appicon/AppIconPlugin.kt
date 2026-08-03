package dev.local.organizer.appicon

import android.app.Activity
import android.content.ComponentName
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
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
    /** The alias the manifest enables on a fresh install. */
    var fallback: String = ""
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
            closeSoTheLauncherRedraws()
        } catch (failure: Exception) {
            invoke.reject(failure.message ?: "не удалось сменить иконку")
        }
    }

    /**
     * Closes the app so the home screen picks the new icon up at once.
     *
     * A launcher reads an app's icon when it is told the package changed, and
     * it is not told while the process is alive — which is why the icon would
     * otherwise appear only after the app was swiped away, or after a reboot.
     * The apps people compare this to all do the same thing.
     *
     * Every component change is already applied by the time this runs, and the
     * choice does not need saving to survive: which alias is enabled is the
     * question's own answer, and Android keeps it.
     */
    private fun closeSoTheLauncherRedraws() {
        // Long enough for the resolved call to reach the WebView, short enough
        // that it reads as the app closing rather than as a freeze.
        Handler(Looper.getMainLooper()).postDelayed({
            activity.finishAndRemoveTask()
            Runtime.getRuntime().exit(0)
        }, CLOSE_DELAY_MILLIS)
    }

    /**
     * The alias currently showing, leaving exactly one of them enabled.
     *
     * An update can end up with two. Component states the app has set are kept
     * across installs, while the manifest decides the rest — so a build that
     * moves the default enables it on top of whatever the user had chosen, and
     * the launcher shows the app twice. Asking which icon is on is also the
     * moment to put that right.
     */
    @Command
    fun currentIcon(invoke: Invoke) {
        val args = invoke.parseArgs(SelectArgs::class.java)
        val enabled = args.known.filter { alias -> isShowing(alias, args.fallback) }

        // The user's own choice is whichever is not the manifest's default, so
        // that survives and the default steps aside. With none enabled the
        // manifest is already the only thing deciding, and nothing is touched.
        val keep = enabled.firstOrNull { it != args.fallback } ?: enabled.firstOrNull()
        if (keep != null && enabled.size > 1) {
            for (other in enabled) {
                if (other != keep) {
                    setEnabled(other, enabled = false)
                }
            }
        }

        invoke.resolve(JSObject().apply { put("alias", keep ?: "") })
    }

    /**
     * Whether an alias is actually offered to the launcher.
     *
     * A component nobody has set reads as `DEFAULT`, which means "whatever the
     * manifest said" — and the manifest enables the default icon. Counting
     * only `ENABLED` would miss exactly the alias that causes the duplicate.
     */
    private fun isShowing(alias: String, fallback: String): Boolean =
        when (activity.packageManager.getComponentEnabledSetting(component(alias))) {
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED -> true
            PackageManager.COMPONENT_ENABLED_STATE_DEFAULT -> alias == fallback
            else -> false
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
            // The process is ended deliberately once every alias has been
            // set, rather than by each call on its way through: being killed
            // half-way would leave two icons enabled, or none.
            PackageManager.DONT_KILL_APP,
        )
    }

    private fun component(alias: String) =
        ComponentName(activity, "dev.local.organizer.appicon.$alias")

    private companion object {
        const val CLOSE_DELAY_MILLIS = 400L
    }
}
