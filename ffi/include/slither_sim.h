/*
 * slither-sim-abi-v2: a C ABI over slither-sim transport sessions.
 *
 * Guide: https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/c-api.md
 * Written by hand from ffi/src/lib.rs; ffi/tests/abi.rs checks that this
 * file's slither_frame layout and constants match the library.
 *
 * Threads: never use one session from two threads at once. Separate sessions
 * may run on separate threads.
 */
#ifndef SLITHER_SIM_H
#define SLITHER_SIM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SLITHER_ABI_VERSION 2u

/* Status codes. */
#define SLITHER_OK 0
/* The session rejected the call and changed nothing; see slither_last_error. */
#define SLITHER_REJECTED 1
/* A required pointer was null; nothing changed. */
#define SLITHER_INVALID 2
/* An internal error; only slither_last_error and slither_session_free remain valid. */
#define SLITHER_PANICKED 3
/* Advancement failed and cleared all worlds; reset to continue. */
#define SLITHER_ENDED 4

/* Row or slot index meaning none: the own row of a dead slot, a missing killer. */
#define SLITHER_NONE UINT32_MAX
/* food_origin codes, in the order of the JSON info reply's origins. */
#define SLITHER_ORIGIN_AMBIENT 0
#define SLITHER_ORIGIN_BOOST 1
#define SLITHER_ORIGIN_DEATH 2
#define SLITHER_NO_ORIGIN 255
/* death_reason codes. */
#define SLITHER_DEATH_BODY 0
#define SLITHER_DEATH_BOUNDARY 1

typedef struct slither_session slither_session;

/*
 * One world's current frame: counts and read-only column pointers, with the
 * values the JSON frame prints. Columns of a table have one element per row;
 * pointers of empty columns must not be dereferenced. Pointers stay valid
 * until the next call that takes the session.
 */
typedef struct slither_frame {
    double time;
    double radius;
    double vision;
    uint32_t ticks;
    uint32_t living;
    size_t slots;
    size_t snakes;
    size_t body;
    size_t food;
    size_t prey;
    size_t deaths;
    /* snakes rows */
    const uint32_t *snake_slot;
    const double *snake_x;
    const double *snake_y;
    const double *snake_heading;
    const double *snake_speed;
    const double *snake_radius;
    const double *snake_length;
    const uint32_t *snake_segments;
    const double *snake_fam;
    const double *snake_score;
    const uint32_t *snake_kills;
    /* snakes + 1 offsets: row r's anchors are body_x[body_start[r]] up to
       body_x[body_start[r + 1]], head to tail; body anchors in all */
    const uint32_t *snake_body_start;
    const double *snake_body_x;
    const double *snake_body_y;
    /* food rows; size is NaN without a size */
    const double *food_x;
    const double *food_y;
    const double *food_value;
    const double *food_size;
    const uint8_t *food_origin;
    /* prey rows */
    const uint64_t *prey_id;
    const double *prey_x;
    const double *prey_y;
    const double *prey_heading;
    const double *prey_target_heading;
    const double *prey_speed;
    const double *prey_size;
    /* slots entries: own snakes row, or SLITHER_NONE when dead */
    const uint32_t *view_self;
    /* slots + 1 offsets each: slot s sees view_snakes[view_snakes_start[s]]
       up to view_snakes[view_snakes_start[s + 1]], and likewise food and prey */
    const uint32_t *view_snakes_start;
    const uint32_t *view_snakes;
    const uint32_t *view_food_start;
    const uint32_t *view_food;
    const uint32_t *view_prey_start;
    const uint32_t *view_prey;
    /* deaths rows during the call that produced this frame */
    const double *death_time;
    const uint32_t *death_victim;
    const uint32_t *death_killer;
    const uint8_t *death_reason;
    const uint32_t *death_food;
} slither_frame;

/* SLITHER_ABI_VERSION of the loaded library. */
uint32_t slither_abi_version(void);

/* A new empty session, or null if it cannot be created. */
slither_session *slither_session_new(void);

/* Frees a session; null is ignored. */
void slither_session_free(slither_session *session);

/*
 * Answers one slither-sim-json-v3 request line of line_len bytes, without its
 * newline, as `slither-sim serve` would. Rejected requests still succeed with
 * an error reply. *reply receives line bytes without a newline, followed by a
 * NUL that *reply_len excludes, valid until the next call that takes the
 * session. line may be null when line_len is 0.
 */
int32_t slither_request(slither_session *session, const uint8_t *line, size_t line_len,
                        const uint8_t **reply, size_t *reply_len);

/*
 * Steps every world like a JSON step, without writing JSON. headings and
 * boosts hold one entry per slot of every world, world-major: slot s of world
 * w is entry w * agents + s. A NaN heading means no action, which only dead
 * slots may give; a nonzero boost boosts. SLITHER_REJECTED changes nothing.
 * SLITHER_ENDED clears all worlds after advancement fails; reset to continue.
 */
int32_t slither_step(slither_session *session, const double *headings, size_t headings_len,
                     const uint8_t *boosts, size_t boosts_len);

/* Fills *frame with world `world`'s frame after the latest reset or step. */
int32_t slither_read_frame(slither_session *session, size_t world, slither_frame *frame);

/* Worlds and agent slots per world of the current session; 0 before reset. */
size_t slither_worlds(const slither_session *session);
size_t slither_agents(const slither_session *session);

/*
 * Why the latest slither_request, slither_step or slither_read_frame call on
 * this session failed, or "" if it succeeded. Valid until the next call that
 * takes the session.
 */
const char *slither_last_error(const slither_session *session);

#ifdef __cplusplus
}
#endif

#endif
