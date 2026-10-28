#define _GNU_SOURCE
#include <assert.h>
#include <dlfcn.h>
#include <linux/perf_event.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>
static int fd=-1; static unsigned scope; static void *mapping; static size_t ring_bytes;
static pid_t owner; static uint64_t period=1000003;
void study_begin(void) {
 if(fd>=0) { close(fd); munmap(mapping,ring_bytes+4096); }
 scope=0; owner=getpid();
 const char *p=getenv("STUDY_SAMPLE_PERIOD"); if(p)period=strtoull(p,0,10);
 struct perf_event_attr a={.type=PERF_TYPE_HARDWARE,.size=sizeof(a),
  .config=PERF_COUNT_HW_INSTRUCTIONS,.sample_period=period,
  .sample_type=PERF_SAMPLE_IP|PERF_SAMPLE_PERIOD,
  .read_format=PERF_FORMAT_TOTAL_TIME_ENABLED|PERF_FORMAT_TOTAL_TIME_RUNNING,
  .disabled=1,.exclude_kernel=1,.exclude_hv=1};
 fd=syscall(__NR_perf_event_open,&a,0,-1,-1,0); assert(fd>=0);
 assert(sysconf(_SC_PAGESIZE)==4096); ring_bytes=512u<<10;
 mapping=mmap(0,ring_bytes+4096,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);if(mapping==MAP_FAILED){perror("perf ring mmap");abort();}
}
unsigned study_scope(unsigned next) {
 unsigned old=scope; scope=next;
 if(fd>=0) {
  if(old==0 && next==2)assert(ioctl(fd,PERF_EVENT_IOC_ENABLE,0)==0);
  if(next==0 && old!=0)assert(ioctl(fd,PERF_EVENT_IOC_DISABLE,0)==0);
 }
 return old;
}
__attribute__((destructor)) static void finish(void) {
 if(fd<0 || owner!=getpid())return;
 assert(ioctl(fd,PERF_EVENT_IOC_DISABLE,0)==0);
 uint64_t counts[3];assert(read(fd,counts,sizeof(counts))==sizeof(counts));
 struct perf_event_mmap_page *meta=mapping;
 uint64_t end=__atomic_load_n(&meta->data_head,__ATOMIC_ACQUIRE);assert(end<=ring_bytes);
 char path[1024]; const char *prefix=getenv("STUDY_LIBM"); assert(prefix);
 assert(snprintf(path,sizeof(path),"%s.samples",prefix)>0);
 FILE *f=fopen(path,"wx");assert(f);
 fprintf(f,"# instructions=%lu enabled=%lu running=%lu period=%lu\n",counts[0],counts[1],counts[2],period);
 fputs("ip,period\n",f);uint64_t samples=0,weight=0;
 for(uint64_t at=0;at<end;) {
  const char *p=(const char *)mapping+meta->data_offset+at;
  struct perf_event_header h;memcpy(&h,p,sizeof(h));assert(h.size && at+h.size<=end);
  assert(h.type!=PERF_RECORD_LOST && h.type!=PERF_RECORD_THROTTLE && h.type!=PERF_RECORD_UNTHROTTLE);
  if(h.type==PERF_RECORD_SAMPLE){uint64_t ip,w; assert(h.size==24);memcpy(&ip,p+8,8);memcpy(&w,p+16,8);fprintf(f,"%lx,%lu\n",ip,w);samples++;weight+=w;}
  else abort();
  at+=h.size;
 }
 assert(fclose(f)==0);assert(counts[1] && counts[1]==counts[2]);
 assert(samples>100);assert((double)weight/counts[0]>.98 && (double)weight/counts[0]<1.02);
 assert(snprintf(path,sizeof(path),"%s.maps",prefix)>0);
 FILE *in=fopen("/proc/self/maps","r"),*out=fopen(path,"wx");assert(in&&out);char line[4096];
 while(fgets(line,sizeof(line),in))fputs(line,out);fclose(in);assert(fclose(out)==0);
 close(fd);
}
