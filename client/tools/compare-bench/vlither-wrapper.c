/* Authored instrumentation, compiled in external scratch only. Stock main.c is
 * included from the user's Git checkout, never copied into this repository.
 * This changes launch/input only, leaves stock rendering and zoom clamps intact,
 * and must be labelled an instrumented build in every result. */
#define tlaunch bench_stock_launch
#define tinit bench_stock_init
#define trender bench_stock_render
#define main bench_unused_main
#include "app/src/main.c"
#undef tlaunch
#undef tinit
#undef trender
#undef main
#include "app/src/network/server.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void tlaunch(tenv* env) {
  bench_stock_launch(env);
  const char* address = getenv("SLITHER_BENCH_SERVER");
  unsigned int port = 0; char tail = 0;
  if (!address || sscanf(address, "127.0.0.1:%u%c", &port, &tail) != 1 ||
      port == 0 || port > 65535 || port == 8766 || port == 18884 || port == 18885 ||
      strlen(address) > MAX_IPV4_LEN) {
    fprintf(stderr, "Vlither wrapper requires an allowed literal loopback server\n");
    exit(2);
  }
  strcpy(env->usr->usrs.ipv4, address);
  strcpy(env->usr->usrs.nickname, "FrameBench");
  env->usr->usrs.vsync = true;
  env->config.vsync = true;
}
void tinit(tenv* env) {
  bench_stock_init(env);
  env->usr->gdata.conn = CONNECTING;
  env->usr->gdata.curr_screen = PLAYING;
  glfwSetTime(0);
  server_connect(env);
}
void trender(tenv* env) {
  static double next = 0;
  game_data* g = &env->usr->gdata;
  /* Exercise stock input() and MAX_ZOOM_OUT; never assign the scale directly. */
  if (getenv("SLITHER_BENCH_WHOLE_MAP") && g->conn == CONNECTED)
    env->ms->dwheel = -1000;
  bench_stock_render(env);
  double now = glfwGetTime();
  if (now >= next && g->conn == CONNECTED) {
    next = now + .5;
    int count = (int)tdarray_length(g->data.foods), visible = 0, uneaten = 0, drawn = 0;
    int fbw = 0, fbh = 0;
    glfwGetFramebufferSize(env->wnd->handle, &fbw, &fbh);
    for (int i = 0; i < count; i++) {
      food* f = &g->data.foods[i];
      if (!f->eaten) uneaten++;
      if (!f->eaten && fabsf(f->rx - g->data.view_xx) * g->data.gsc <= env->ctx->size[0] / 2.f &&
          fabsf(f->ry - g->data.view_yy) * g->data.gsc <= env->ctx->size[1] / 2.f)
        visible++;
    }
    for (int i = 0; i < (int)tdarray_length(g->data.snakes); i++) {
      snake* s = &g->data.snakes[i];
      if (!s->dead && s->iiv && s->alive_amt > 0 && s->dead_amt < 1) drawn++;
    }
    /* gsc maps world offsets to ctx framebuffer pixels in stock redraw.c.
     * The runner also checks the actual framebuffer and renderer dimensions,
     * then an independent host-anchored picture before matching is accepted.
     * Stock wheel: app/src/game/input.c and thermite/src/framework/tmouse.c
     * at https://github.com/for-loop9/vlither/tree/8612bc12186547fc385ea38a399b144973338384 */
    printf("frame-bench-client: {\"schema\":1,\"instrumented\":true,\"connected\":true,"
      "\"alive\":%s,\"camera\":[%.9g,%.9g],\"center\":%.9g,\"radius\":%.9g,"
      "\"zoom\":%.9g,\"viewport\":[%d,%d],\"retainedPellets\":%d,\"visiblePellets\":%d,"
      "\"receivedUneatenPellets\":%d,\"drawnLiveSnakes\":%d,"
      "\"drawnSnakeScope\":\"stock redraw visibility flag and live opacity\","
      "\"zoomUnits\":\"framebuffer-pixels-per-world-unit\",\"framebuffer\":[%d,%d],\"rendererViewport\":[%d,%d],"
      "\"wheelZoom\":%.9g,\"wheelStep\":%.9g,"
      "\"protocol\":%d,\"stockZoomMinimum\":%.9g,\"stockZoomMaximum\":%.9g,\"presentMode\":\"FIFO\"}\n",
      g->data.dead ? "false" : "true", g->data.view_xx, g->data.view_yy,
      g->data.grd, fmaxf(g->data.flux_grd, g->data.real_flux_grd), g->data.gsc,
      env->ctx->size[0], env->ctx->size[1], count, visible,
      uneaten, drawn, fbw, fbh, env->usr->r->size[0], env->usr->r->size[1],
      g->data.ms_zoom, env->usr->usrs.zoom_step,
      g->data.protocol_version, (double)MAX_ZOOM_OUT, (double)MAX_ZOOM_IN);
    fflush(stdout);
  }
}
int main(int argc, char** argv) { return tentry(argc, argv, sizeof(tuser_data)); }
