import java.io.File
import javax.inject.Inject
import org.apache.tools.ant.taskdefs.condition.Os
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.logging.LogLevel
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

abstract class BuildTask @Inject constructor(
    private val execOperations: ExecOperations,
) : DefaultTask() {
    @Input
    var rootDirRel: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    @TaskAction
    fun assemble() {
        val executable = "pnpm"
        try {
            runTauriCli(executable)
        } catch (error: Exception) {
            if (Os.isFamily(Os.FAMILY_WINDOWS)) {
                val fallbacks = listOf(
                    "$executable.exe",
                    "$executable.cmd",
                    "$executable.bat",
                )

                var lastException: Exception = error
                for (fallback in fallbacks) {
                    try {
                        runTauriCli(fallback)
                        return
                    } catch (fallbackException: Exception) {
                        lastException = fallbackException
                    }
                }
                throw lastException
            }
            throw error
        }
    }

    private fun runTauriCli(executable: String) {
        val rootDirRel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        val target = target ?: throw GradleException("target cannot be null")
        val release = release ?: throw GradleException("release cannot be null")
        val commandArgs = mutableListOf("tauri", "android", "android-studio-script")

        if (project.logger.isEnabled(LogLevel.DEBUG)) {
            commandArgs.add("-vv")
        } else if (project.logger.isEnabled(LogLevel.INFO)) {
            commandArgs.add("-v")
        }
        if (release) {
            commandArgs.add("--release")
        }
        commandArgs.addAll(listOf("--target", target))

        execOperations.exec {
            workingDir(File(project.projectDir, rootDirRel))
            commandLine(listOf(executable) + commandArgs)
        }.assertNormalExitValue()
    }
}
