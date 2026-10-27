#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/tcp.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

static uint64_t ns(clockid_t id) {
    struct timespec t;
    if (clock_gettime(id, &t)) { perror("clock"); exit(2); }
    return (uint64_t)t.tv_sec * 1000000000ULL + t.tv_nsec;
}
static void pin(int cpu) {
    cpu_set_t cpus;
    CPU_ZERO(&cpus); CPU_SET(cpu, &cpus);
    if (sched_setaffinity(0, sizeof(cpus), &cpus)) { perror("affinity"); exit(2); }
}
static void die(const char *s) { perror(s); exit(2); }

static int null_bench(uint64_t count) {
    pin(0);
    for (unsigned i = 0; i < 100000; i++) if (syscall(SYS_getppid) <= 0) die("getppid");
    uint64_t c = ns(CLOCK_THREAD_CPUTIME_ID), w = ns(CLOCK_MONOTONIC_RAW);
    for (uint64_t i = 0; i < count; i++) __asm__ volatile("" ::: "memory");
    uint64_t bw = ns(CLOCK_MONOTONIC_RAW)-w, bc = ns(CLOCK_THREAD_CPUTIME_ID)-c;
    c = ns(CLOCK_THREAD_CPUTIME_ID); w = ns(CLOCK_MONOTONIC_RAW);
    uint64_t errors = 0;
    for (uint64_t i = 0; i < count; i++) errors += syscall(SYS_getppid) <= 0;
    uint64_t wall = ns(CLOCK_MONOTONIC_RAW)-w, cpu = ns(CLOCK_THREAD_CPUTIME_ID)-c;
    printf("{\"calls\":%llu,\"cpuNs\":%llu,\"wallNs\":%llu,\"baselineCpuNs\":%llu,\"baselineWallNs\":%llu,\"errors\":%llu}\n",
        (unsigned long long)count, (unsigned long long)cpu, (unsigned long long)wall,
        (unsigned long long)bc, (unsigned long long)bw, (unsigned long long)errors);
    return errors ? 2 : 0;
}

struct sends { uint64_t calls, successful, bytes, partial, eagain, errors; };
static struct sends send_loop(int *fds, unsigned n, uint64_t calls) {
    const unsigned char payload[22] = {0};
    struct sends result = {.calls = calls};
    for (uint64_t i = 0; i < calls; i++) {
        ssize_t sent = send(fds[i % n], payload, sizeof(payload), MSG_NOSIGNAL);
        if (sent == 22) result.successful++;
        else if (sent >= 0) result.partial++;
        else if (errno == EAGAIN || errno == EWOULDBLOCK) result.eagain++;
        else result.errors++;
        if (sent > 0) result.bytes += sent;
    }
    return result;
}
static int send_bench(const char *address, int port, unsigned sockets, uint64_t warm, uint64_t calls) {
    pin(0);
    int *fds = calloc(sockets, sizeof(int));
    if (!fds) die("calloc");
    struct sockaddr_in addr = {.sin_family = AF_INET, .sin_port = htons(port)};
    if (inet_pton(AF_INET, address, &addr.sin_addr) != 1) return 2;
    for (unsigned i = 0; i < sockets; i++) {
        fds[i] = socket(AF_INET, SOCK_STREAM | SOCK_CLOEXEC, 0);
        if (fds[i] < 0) die("socket");
        int one = 1;
        if (setsockopt(fds[i], IPPROTO_TCP, TCP_NODELAY, &one, sizeof(one))) die("nodelay");
        if (connect(fds[i], (struct sockaddr *)&addr, sizeof(addr))) die("connect");
        if (fcntl(fds[i], F_SETFL, O_NONBLOCK)) die("nonblock");
    }
    struct sends a = send_loop(fds, sockets, warm);
    uint64_t c = ns(CLOCK_THREAD_CPUTIME_ID), w = ns(CLOCK_MONOTONIC_RAW);
    struct sends b = send_loop(fds, sockets, calls);
    uint64_t wall = ns(CLOCK_MONOTONIC_RAW)-w, cpu = ns(CLOCK_THREAD_CPUTIME_ID)-c;
    for (unsigned i = 0; i < sockets; i++) close(fds[i]);
    free(fds);
    printf("{\"sockets\":%u,\"calls\":%llu,\"successful\":%llu,\"cpuNs\":%llu,\"wallNs\":%llu,\"partial\":%llu,\"eagain\":%llu,\"errors\":%llu,\"warmupErrors\":%llu,\"totalBytesSent\":%llu}\n",
        sockets, (unsigned long long)b.calls, (unsigned long long)b.successful,
        (unsigned long long)cpu, (unsigned long long)wall, (unsigned long long)b.partial,
        (unsigned long long)b.eagain, (unsigned long long)b.errors,
        (unsigned long long)(a.partial+a.eagain+a.errors), (unsigned long long)(a.bytes+b.bytes));
    return (a.partial+a.eagain+a.errors+b.partial+b.eagain+b.errors) ? 2 : 0;
}

static atomic_uint ready, accepted, closed_count;
static atomic_ullong received;
static atomic_int stop_sink;
static unsigned expected;
static int sink_port;
struct sink_result { uint64_t cpu, wall, reads, bytes; };
static struct sink_result results[4];
static void *sink_thread(void *arg) {
    int index = (int)(intptr_t)arg;
    pin(index);
    int listener = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0), one = 1;
    if (listener < 0) die("listener");
    if (setsockopt(listener, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one)) ||
        setsockopt(listener, SOL_SOCKET, SO_REUSEPORT, &one, sizeof(one))) die("reuseport");
    struct sockaddr_in addr = {.sin_family=AF_INET, .sin_port=htons(sink_port), .sin_addr.s_addr=INADDR_ANY};
    if (bind(listener, (struct sockaddr *)&addr, sizeof(addr)) || listen(listener, 512)) die("listen");
    int ep = epoll_create1(EPOLL_CLOEXEC);
    if (ep < 0) die("epoll");
    struct epoll_event interest = {.events=EPOLLIN, .data.fd=listener};
    if (epoll_ctl(ep, EPOLL_CTL_ADD, listener, &interest)) die("epoll listener");
    struct sink_result *r = &results[index];
    uint64_t cpu = ns(CLOCK_THREAD_CPUTIME_ID), wall = ns(CLOCK_MONOTONIC_RAW);
    atomic_fetch_add(&ready, 1);
    while (!atomic_load(&stop_sink)) {
        struct epoll_event events[128];
        int n = epoll_wait(ep, events, 128, 100);
        if (n < 0) { if (errno == EINTR) continue; die("epoll_wait"); }
        for (int i = 0; i < n; i++) {
            int fd = events[i].data.fd;
            if (fd == listener) {
                for (;;) {
                    int client = accept4(listener, NULL, NULL, SOCK_NONBLOCK | SOCK_CLOEXEC);
                    if (client < 0) { if (errno == EAGAIN || errno == EWOULDBLOCK) break; die("accept"); }
                    struct epoll_event event = {.events=EPOLLIN | EPOLLRDHUP, .data.fd=client};
                    if (epoll_ctl(ep, EPOLL_CTL_ADD, client, &event)) die("epoll client");
                    atomic_fetch_add(&accepted, 1);
                }
            } else {
                for (;;) {
                    char buffer[65536];
                    ssize_t got = recv(fd, buffer, sizeof(buffer), 0);
                    r->reads++;
                    if (got > 0) { r->bytes += got; atomic_fetch_add(&received, got); }
                    else if (!got) {
                        if (epoll_ctl(ep, EPOLL_CTL_DEL, fd, NULL)) die("epoll delete");
                        close(fd);
                        if (atomic_fetch_add(&closed_count, 1)+1 == expected) atomic_store(&stop_sink, 1);
                        break;
                    } else if (errno == EAGAIN || errno == EWOULDBLOCK) break;
                    else if (errno != EINTR) die("recv");
                }
            }
        }
    }
    r->wall = ns(CLOCK_MONOTONIC_RAW)-wall; r->cpu = ns(CLOCK_THREAD_CPUTIME_ID)-cpu;
    close(ep); close(listener);
    return NULL;
}
static int sink(int port, unsigned connections, const char *output) {
    sink_port = port; expected = connections;
    pthread_t threads[4];
    for (int i=0; i<4; i++) if (pthread_create(&threads[i], NULL, sink_thread, (void *)(intptr_t)i)) return 2;
    while (atomic_load(&ready) != 4) usleep(1000);
    char *path = malloc(strlen(output)+7);
    if (!path) die("path");
    sprintf(path, "%s.ready", output);
    FILE *f = fopen(path, "w"); if (!f) die("ready"); fputs("ready\n", f); fclose(f); free(path);
    for (int i=0; i<4; i++) pthread_join(threads[i], NULL);
    f = fopen(output, "w"); if (!f) die("output");
    fprintf(f, "{\"accepted\":%u,\"closed\":%u,\"bytes\":%llu,\"threads\":[",
        atomic_load(&accepted), atomic_load(&closed_count), (unsigned long long)atomic_load(&received));
    for (int i=0; i<4; i++) fprintf(f, "%s{\"cpuNs\":%llu,\"wallNs\":%llu,\"reads\":%llu,\"bytes\":%llu}",
        i ? "," : "", (unsigned long long)results[i].cpu, (unsigned long long)results[i].wall,
        (unsigned long long)results[i].reads, (unsigned long long)results[i].bytes);
    fputs("]}\n", f); fclose(f);
    return atomic_load(&accepted) == expected ? 0 : 2;
}
int main(int argc, char **argv) {
    if (argc==3 && !strcmp(argv[1], "null")) return null_bench(strtoull(argv[2], NULL, 10));
    if (argc==7 && !strcmp(argv[1], "send")) return send_bench(argv[2], atoi(argv[3]), atoi(argv[4]),
        strtoull(argv[5], NULL, 10), strtoull(argv[6], NULL, 10));
    if (argc==5 && !strcmp(argv[1], "sink")) return sink(atoi(argv[2]), atoi(argv[3]), argv[4]);
    fputs("Usage: micro null N | micro send ADDRESS PORT SOCKETS WARM CALLS | micro sink PORT SOCKETS OUTPUT\n", stderr);
    return 2;
}
