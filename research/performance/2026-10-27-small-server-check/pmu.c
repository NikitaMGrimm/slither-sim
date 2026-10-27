/* Research-only own-child Linux user instructions, including inherited threads.
 * Enable on exec excludes the wrapper and pre-exec pipe handshake. */
#include <linux/perf_event.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <sys/resource.h>
#include <unistd.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
    if (argc < 3) return 2;
    int pipefd[2];
    if (pipe(pipefd)) { perror("pipe"); return 2; }
    pid_t pid = fork();
    if (pid < 0) { perror("fork"); return 2; }
    if (!pid) {
        close(pipefd[1]);
        char byte;
        if (read(pipefd[0], &byte, 1) != 1) _exit(2);
        close(pipefd[0]);
        execvp(argv[2], argv + 2);
        perror("execvp"); _exit(2);
    }
    close(pipefd[0]);
    struct perf_event_attr attr = {0};
    attr.size = sizeof(attr); attr.type = PERF_TYPE_HARDWARE;
    attr.config = PERF_COUNT_HW_INSTRUCTIONS;
    attr.disabled = 1; attr.inherit = 1; attr.enable_on_exec = 1;
    attr.exclude_kernel = 1; attr.exclude_hv = 1;
    attr.read_format = PERF_FORMAT_TOTAL_TIME_ENABLED | PERF_FORMAT_TOTAL_TIME_RUNNING;
    int fd = syscall(__NR_perf_event_open, &attr, pid, -1, -1, 0);
    if (fd < 0) { perror("perf_event_open"); close(pipefd[1]); waitpid(pid, NULL, 0); return 2; }
    if (write(pipefd[1], "x", 1) != 1) return 2;
    close(pipefd[1]);
    int status;
    struct rusage usage;
    if (wait4(pid, &status, 0, &usage) != pid) return 2;
    uint64_t v[3];
    if (read(fd, v, sizeof(v)) != sizeof(v)) { perror("counter read"); return 2; }
    close(fd);
    FILE *out = fopen(argv[1], "w");
    if (!out) { perror("fopen"); return 2; }
    fprintf(out, "{\"instructions\":%llu,\"enabled\":%llu,\"running\":%llu,\"userSeconds\":%.6f,\"kernelSeconds\":%.6f}\n",
            (unsigned long long)v[0], (unsigned long long)v[1], (unsigned long long)v[2],
            usage.ru_utime.tv_sec + usage.ru_utime.tv_usec / 1e6,
            usage.ru_stime.tv_sec + usage.ru_stime.tv_usec / 1e6);
    if (fclose(out)) return 2;
    return WIFEXITED(status) ? WEXITSTATUS(status) : 2;
}
