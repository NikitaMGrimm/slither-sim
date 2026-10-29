/* Own-child, x86-64 Linux diagnostic. No attach or system settings. */
#define _GNU_SOURCE
#include <sys/ptrace.h>
#include <sys/wait.h>
#include <sys/user.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <signal.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct thread { pid_t pid; int entering, writing, sends; } threads[128];
struct row { uint64_t tick, messages, calls, flushes, sendto, write, writev, sendmsg, outside, good, bad; } row;
static int count;
static struct thread *find(pid_t pid) {
    for(int i=0;i<count;i++) if(threads[i].pid==pid) return &threads[i];
    if(count==128) abort();
    threads[count]=(struct thread){.pid=pid,.entering=1};
    return &threads[count++];
}
static void emit(FILE *out) {
    fprintf(out,"%lu,%lu,%lu,%lu,%lu,%lu,%lu,%lu,%lu,%lu,%lu\n",row.tick,row.messages,row.calls,row.flushes,row.sendto,row.write,row.writev,row.sendmsg,row.outside,row.good,row.bad);
    fflush(out);
}
int main(int argc,char **argv) {
    if(argc<4) return 2;
    FILE *out=fopen(argv[1],"wx"); if(!out) return 3;
    fprintf(out,"tick,messages,tcp_write_calls,open_flushes,sendto,write,writev,sendmsg,sends_outside_tcp_write,one_sendto_calls,other_calls\n");
    pid_t child=fork(); if(child<0) return 4;
    if(!child){
        if(ptrace(PTRACE_TRACEME,0,0,0)) _exit(90);
        raise(SIGSTOP); execvp(argv[2],argv+2); _exit(91);
    }
    int status; if(waitpid(child,&status,0)!=child||!WIFSTOPPED(status)) return 5;
    unsigned long opts=PTRACE_O_TRACESYSGOOD|PTRACE_O_TRACECLONE|PTRACE_O_TRACEEXEC|PTRACE_O_EXITKILL;
    if(ptrace(PTRACE_SETOPTIONS,child,0,opts)) {perror("ptrace options");return 6;}
    find(child); ptrace(PTRACE_SYSCALL,child,0,0);
    while(1){
        pid_t pid=waitpid(-1,&status,__WALL);
        if(pid<0){if(errno==EINTR)continue;if(errno==ECHILD)break;return 7;}
        struct thread *thread=find(pid);
        if(WIFEXITED(status)||WIFSIGNALED(status)){thread->pid=-1;continue;}
        if(!WIFSTOPPED(status)) return 8;
        int sig=WSTOPSIG(status),deliver=0;
        unsigned event=(unsigned)status>>16;
        if(sig==(SIGTRAP|0x80)){
            struct __ptrace_syscall_info info;
            long n=ptrace(PTRACE_GET_SYSCALL_INFO,pid,sizeof(info),&info);
            if(n<0){perror("syscall info");return 9;}
            if(info.op==PTRACE_SYSCALL_INFO_ENTRY){
                uint64_t call=info.entry.nr,fd=info.entry.args[0];
                if(call==SYS_write && (int32_t)fd==-701){
                    emit(out); memset(&row,0,sizeof(row));row.tick=info.entry.args[1];
                } else if(call==SYS_write && (int32_t)fd==-700){
                    switch(info.entry.args[1]){
                    case 0:row.messages++;break;
                    case 1:if(thread->writing)abort();row.calls++;thread->writing=1;thread->sends=0;break;
                    case 2:row.flushes++;break;
                    case 3:if(!thread->writing)abort();if(thread->sends==1)row.good++;else row.bad++;thread->writing=0;break;
                    default:abort();
                    }
                } else if(call==SYS_sendto || call==SYS_sendmsg || call==SYS_write || call==SYS_writev){
                    /* Count only transport operations bracketed by the Rust
                       TcpStream Write wrapper; other writes include logs. */
                    if(thread->writing){
                        if(call==SYS_sendto)thread->sends++;else thread->sends+=1000;
                        if(call==SYS_sendto)row.sendto++;
                        if(call==SYS_sendmsg)row.sendmsg++;
                        if(call==SYS_write)row.write++;
                        if(call==SYS_writev)row.writev++;
                    } else if(call==SYS_sendto || call==SYS_sendmsg)row.outside++;
                }
            }
        } else if(sig==SIGTRAP && event){
            if(event==PTRACE_EVENT_CLONE){unsigned long next=0;if(ptrace(PTRACE_GETEVENTMSG,pid,0,&next))return 10;find((pid_t)next);}
        } else if(sig!=SIGSTOP) deliver=sig;
        if(ptrace(PTRACE_SYSCALL,pid,0,deliver) && errno!=ESRCH){perror("resume");return 11;}
    }
    emit(out); fclose(out);return 0;
}
