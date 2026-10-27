/*
 * slither-sim-abi-v3: a C ABI over slither-sim transport sessions.
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

#define SLITHER_ABI_VERSION 3u

/* Status codes. */
#define SLITHER_OK 0
/* The session rejected the call and changed nothing; see slither_last_error. */
#define SLITHER_REJECTED 1
/* A required pointer was null; nothing changed. */
#define SLITHER_INVALID 2
/* An internal error; only slither_last_error, slither_read_error and slither_session_free remain valid. */
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
#define SLITHER_DEATH_PIT 2

typedef struct slither_session slither_session;

/*
 * One world's current frame: counts and read-only column pointers, with the
 * values the JSON frame prints. Columns of a table have one element per row;
 * pointers of empty columns must not be dereferenced. Pointers stay valid
 * until the next call that takes the session.
 */
#define SLITHER_BOUNDARY_SLIDING 0
#define SLITHER_BOUNDARY_HARD 1
#define SLITHER_RULE_HAS_PIT 1
#define SLITHER_RULE_HAS_MIN_SCORE 2
#define SLITHER_RULE_HAS_KILL_WINDOW 4
#define SLITHER_RULE_FREE_BOOST_AT_FLOOR 8
#define SLITHER_CAUSE_NONE 0
#define SLITHER_CAUSE_INVALID_REQUEST 1
#define SLITHER_CAUSE_INVALID_CONFIG 2
#define SLITHER_CAUSE_ACTION_SHAPE 3
#define SLITHER_CAUSE_INVALID_ACTION 4
#define SLITHER_CAUSE_UNKNOWN_SLOT 5
#define SLITHER_CAUSE_UNKNOWN_WORLD 6
#define SLITHER_CAUSE_UNKNOWN_ENTITY 7
#define SLITHER_CAUSE_DUPLICATE_ID 8
#define SLITHER_CAUSE_INVALID_STATE 9
#define SLITHER_CAUSE_PLACEMENT_EXHAUSTED 10
#define SLITHER_CAUSE_NOT_RESET 11
#define SLITHER_CAUSE_WORLD_STOPPED 12
#define SLITHER_CAUSE_UNSUPPORTED_CHECKPOINT 13
#define SLITHER_CAUSE_INVALID_CHECKPOINT 14
#define SLITHER_CAUSE_NUMERIC_OVERFLOW 15
#define SLITHER_CAUSE_REPRESENTATION_LIMIT 16
#define SLITHER_CAUSE_ALLOCATION_FAILED 17
#define SLITHER_CONSTRAINT_NONE 0
#define SLITHER_CONSTRAINT_MISSING 1
#define SLITHER_CONSTRAINT_NON_FINITE 2
#define SLITHER_CONSTRAINT_NEGATIVE 3
#define SLITHER_CONSTRAINT_NON_POSITIVE 4
#define SLITHER_CONSTRAINT_OUT_OF_RANGE 5
#define SLITHER_CONSTRAINT_LENGTH_MISMATCH 6
#define SLITHER_CONSTRAINT_DUPLICATE 7
#define SLITHER_CONSTRAINT_ORDERING 8
#define SLITHER_CONSTRAINT_OUTSIDE_ARENA 9
#define SLITHER_CONSTRAINT_INCONSISTENT_STATE 10
#define SLITHER_ERROR_HAS_WORLD 1
#define SLITHER_ERROR_HAS_SLOT 2
#define SLITHER_CAUSE_INVALID_POINTER 18
#define SLITHER_CAUSE_PANIC 19

typedef struct slither_world_rules {
    uint32_t boundary_mode;
    uint32_t flags;
    double pit_radius;
    uint32_t min_score;
    double kill_score_seconds;
} slither_world_rules;

/* code 0 means no error; rule 0 and NULL field mean absent.
 * Location flags distinguish absent values from every valid size_t value.
 * Strings remain valid until the next request/step/frame read on this session. */
typedef struct slither_error {
    uint32_t code;
    uint32_t rule;
    uint32_t flags;
    size_t world;
    size_t slot;
    const char *field;
    const char *message;
} slither_error;

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
    /* kills inside the config's killScoreSeconds, across lives; 0 without it */
    const uint32_t *snake_kill_score;
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
    slither_world_rules rules;
} slither_frame;

/* SLITHER_ABI_VERSION of the loaded library. */
uint32_t slither_abi_version(void);

/* A new empty session, or null if it cannot be created. */
slither_session *slither_session_new(void);

/* Frees a session; null is ignored. */
void slither_session_free(slither_session *session);

/*
 * Answers one slither-sim-json-v6 request line of line_len bytes, without its
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

/* Read the latest cause without clearing it, including embedded JSON errors. */
int32_t slither_read_error(const slither_session *session, slither_error *error);

#ifdef __cplusplus
}
#endif

#endif
