#define _GNU_SOURCE
#include <dlfcn.h>
#include <linux/perf_event.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

/* Research-only, single-thread fixture. A common no-IPA measurement routine
 * calls either libm or a one-ret control with the identical surrounding code.
 * Subtract the stable control bracket, then add its one retired instruction.
 * Raw brackets and calibration remain available for independent inspection. */
static _Thread_local unsigned scope;
static int fd = -1;
struct row { void *pc; const char *name; unsigned scope; uint64_t calls, ins, corrected; };
static struct row rows[8192];
static uint64_t control1, control2, control_sc;
static uint64_t initial[3], latest[3];
static char executable[PATH_MAX];
unsigned study_scope(unsigned next) { unsigned old=scope; scope=next; return old; }
static uint64_t counter(void) {
    if(fd < 0) {
        struct perf_event_attr a={.type=PERF_TYPE_HARDWARE,.size=sizeof(a),
            .config=PERF_COUNT_HW_INSTRUCTIONS,.exclude_kernel=1,.exclude_hv=1,
            .read_format=PERF_FORMAT_TOTAL_TIME_ENABLED|PERF_FORMAT_TOTAL_TIME_RUNNING};
        fd=syscall(SYS_perf_event_open,&a,0,-1,-1,0);
        if(fd < 0) abort();
    }
    if(read(fd,latest,24)!=24) abort(); return latest[0];
}
struct measured {double value; uint64_t ins;};
/* noipa prevents a specialized control clone from changing the bracket code. */
__attribute__((noipa)) static struct measured measure1(double (*f)(double), double x) {
    uint64_t a=counter(); double v=f(x); uint64_t b=counter();
    return (struct measured){v,b-a};
}
__attribute__((noipa)) static struct measured measure2(double (*f)(double,double), double x,double y) {
    uint64_t a=counter(); double v=f(x,y); uint64_t b=counter();
    return (struct measured){v,b-a};
}
__attribute__((noipa)) static uint64_t measure_sc(void (*f)(double,double*,double*),double x,double *s,double *c) {
    uint64_t a=counter(); f(x,s,c); uint64_t b=counter(); return b-a;
}
__attribute__((noipa,used)) static double empty1(double x) { return x; }
__attribute__((noipa,used)) static double empty2(double x,double y) { return x; }
__attribute__((noipa,used)) static void empty_sc(double x,double *s,double *c) { }
__attribute__((noipa,used)) static double probe1(double x) {
    __asm__ volatile(".rept 32; nop; .endr"); return x;
}
__attribute__((noipa,used)) static double probe2(double x,double y) {
    __asm__ volatile(".rept 32; nop; .endr"); return x;
}
__attribute__((noipa,used)) static void probe_sc(double x,double *s,double *c) {
    __asm__ volatile(".rept 32; nop; .endr");
}
__attribute__((constructor)) static void calibrate(void) {
    if(!getenv("STUDY_LIBM")) return;
    ssize_t n=readlink("/proc/self/exe",executable,sizeof(executable)-1);
    if(n<=0 || n>=(ssize_t)sizeof(executable)-1) abort(); executable[n]=0;
    counter(); for(unsigned i=0;i<3;i++) initial[i]=latest[i]; double s=0,c=0;
    control1=measure1(empty1,1).ins;
    control2=measure2(empty2,1,2).ins;
    control_sc=measure_sc(empty_sc,1,&s,&c);
    for(unsigned i=0;i<64;i++) {
        if(measure1(empty1,1).ins!=control1 || measure2(empty2,1,2).ins!=control2 ||
           measure_sc(empty_sc,1,&s,&c)!=control_sc) abort();
    }
    if(measure1(probe1,1).ins-control1+1!=33 ||
       measure2(probe2,1,2).ins-control2+1!=33 ||
       measure_sc(probe_sc,1,&s,&c)-control_sc+1!=33) abort();
}
/* A fork child must count its own thread, not the inherited parent's event. */
void study_begin(void) {
    scope=0;
    if(fd>=0) close(fd);
    fd=-1;
    memset(rows,0,sizeof(rows));
    calibrate();
}
static void record(void *pc,const char *name,unsigned s,uint64_t ins,uint64_t control) {
    if(ins<control) abort();
    unsigned i=(((uintptr_t)pc >> 3) ^ (s*257)) % 8192;
    while(rows[i].pc && (rows[i].pc!=pc || rows[i].scope!=s)) i=(i+1)%8192;
    rows[i].pc=pc; rows[i].name=name; rows[i].scope=s;
    rows[i].calls++; rows[i].ins+=ins; rows[i].corrected+=ins-control+1;
}
#define TWO(name) \
double name(double x,double y) { \
    static double (*real)(double,double); \
    if(!real) { *(void **)(&real)=dlsym(RTLD_NEXT,#name); if(!real) abort(); } \
    unsigned s=scope; if(!s) return real(x,y); \
    void *pc=__builtin_return_address(0); struct measured v=measure2(real,x,y); \
    record(pc,#name,s,v.ins,control2); return v.value; }
#define ONE(name) \
double name(double x) { \
    static double (*real)(double); \
    if(!real) { *(void **)(&real)=dlsym(RTLD_NEXT,#name); if(!real) abort(); } \
    unsigned s=scope; if(!s) return real(x); \
    void *pc=__builtin_return_address(0); struct measured v=measure1(real,x); \
    record(pc,#name,s,v.ins,control1); return v.value; }
TWO(hypot) TWO(fmod) TWO(atan2) TWO(pow)
ONE(asin) ONE(sin) ONE(cos) ONE(atan) ONE(exp) ONE(log) ONE(sqrt)
void sincos(double x,double *sn,double *cs) {
    static void (*real)(double,double*,double*);
    if(!real) { *(void **)(&real)=dlsym(RTLD_NEXT,"sincos"); if(!real) abort(); }
    unsigned s=scope; if(!s) {real(x,sn,cs); return;}
    void *pc=__builtin_return_address(0);
    uint64_t ins=measure_sc(real,x,sn,cs); record(pc,"sincos",s,ins,control_sc);
}
__attribute__((destructor)) static void finish(void) {
    const char *path=getenv("STUDY_LIBM"); if(!path) return;
    counter(); uint64_t enabled=latest[1]-initial[1], running=latest[2]-initial[2];
    if(!enabled || (double)running/(double)enabled < .99) abort();
    FILE *f=fopen(path,"wx"); if(!f) abort();
    fprintf(f,"# controls=%lu,%lu,%lu; probes=33,33,33; enabled=%lu; running=%lu; empty functions each retire one ret\n",control1,control2,control_sc,enabled,running);
    fputs("scope,function,offset,calls,instructions,callee_instructions\n",f);
    for(unsigned i=0;i<8192;i++) if(rows[i].pc) {
        Dl_info d; if(!dladdr(rows[i].pc,&d)) abort();
        /* Reject nested interposition or an unsupported non-executable caller;
         * never symbolize a shared-library offset as a runtime offset. */
        if(strcmp(d.dli_fname,executable)) abort();
        fprintf(f,"%u,%s,%lx,%lu,%lu,%lu\n",rows[i].scope,rows[i].name,
            (unsigned long)((uintptr_t)rows[i].pc-(uintptr_t)d.dli_fbase),
            rows[i].calls,rows[i].ins,rows[i].corrected);
    }
    fclose(f);
}
