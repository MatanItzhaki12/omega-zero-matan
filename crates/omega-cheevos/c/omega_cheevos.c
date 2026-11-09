/* omega-zero's side of rcheevos: one rc_client for the handheld, driven from Rust through the
 * oc_* calls below. Rust serialises every call, so nothing here locks. The client's requests go
 * out through omega_cheevos_server_call, which Rust answers later with oc_answer, and what
 * happens in a game comes back through omega_cheevos_event. The game's memory is read through
 * rc_libretro, from the map the core declared or the regions it answers for by number. A disc's
 * files are read for its hash by rcheevos' own reader, but a compressed PSP image (.cso), which it
 * cannot read, is read sector by sector through omega_cheevos_cso_*. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "rc_api_runtime.h"
#include "rc_client.h"
#include "rc_hash.h"
#include "rc_libretro.h"

extern void omega_cheevos_server_call(uint32_t id, const char* url, const char* post_data,
                                      const char* content_type);
extern void omega_cheevos_event(uint32_t kind, const char* title, const char* description,
                                uint32_t points);
extern void* omega_cheevos_cso_open(const char* path);
extern size_t omega_cheevos_cso_read(void* cso, uint32_t sector, void* buffer, size_t bytes);
extern void omega_cheevos_cso_close(void* cso);

/* rcheevos' own reader, for every disc image but a .cso */
static struct rc_hash_cdreader disc_reader;

/* a track opened by either reader */
typedef struct track_t {
  int cso;
  void* handle;
} track_t;

static int is_cso(const char* path) {
  const char* dot = strrchr(path, '.');
  if (!dot || strlen(dot) != 4)
    return 0;
  const char* want = ".cso";
  for (int i = 0; i < 4; i++) {
    char ch = dot[i];
    if (ch >= 'A' && ch <= 'Z')
      ch = (char)(ch - 'A' + 'a');
    if (ch != want[i])
      return 0;
  }
  return 1;
}

static void* RC_CCONV open_track(const char* path, uint32_t track,
                                 const struct rc_hash_iterator* iterator) {
  void* handle = is_cso(path) ? omega_cheevos_cso_open(path)
                              : disc_reader.open_track_iterator(path, track, iterator);
  if (!handle)
    return NULL;
  track_t* t = (track_t*)malloc(sizeof(track_t));
  if (!t) {
    is_cso(path) ? omega_cheevos_cso_close(handle) : disc_reader.close_track(handle);
    return NULL;
  }
  t->cso = is_cso(path);
  t->handle = handle;
  return t;
}

static size_t RC_CCONV read_sector(void* track, uint32_t sector, void* buffer, size_t bytes) {
  track_t* t = (track_t*)track;
  return t->cso ? omega_cheevos_cso_read(t->handle, sector, buffer, bytes)
                : disc_reader.read_sector(t->handle, sector, buffer, bytes);
}

static void RC_CCONV close_track(void* track) {
  track_t* t = (track_t*)track;
  if (!t)
    return;
  if (t->cso)
    omega_cheevos_cso_close(t->handle);
  else
    disc_reader.close_track(t->handle);
  free(t);
}

static uint32_t RC_CCONV first_track_sector(void* track) {
  track_t* t = (track_t*)track;
  /* a .cso is one data track of 2048-byte sectors from the start */
  return t->cso ? 0 : disc_reader.first_track_sector(t->handle);
}

static void use_disc_readers(void) {
  struct rc_hash_cdreader reader;
  rc_hash_get_default_cdreader(&disc_reader);
  memset(&reader, 0, sizeof(reader));
  reader.open_track_iterator = open_track;
  reader.read_sector = read_sector;
  reader.close_track = close_track;
  reader.first_track_sector = first_track_sector;
  rc_hash_init_custom_cdreader(&reader);
}

static rc_client_t* client;

static rc_libretro_memory_regions_t regions;
static int regions_ready;
static uint8_t* system_ram;
static size_t system_len;
static uint8_t* save_ram;
static size_t save_len;
static uint8_t* video_ram;
static size_t video_len;

#define MAX_PENDING 32
typedef struct pending_t {
  uint32_t id;
  rc_client_server_callback_t callback;
  void* data;
} pending_t;
static pending_t pending[MAX_PENDING];
static uint32_t next_id = 1;

static int login_state; /* 0 none, 1 under way, 2 signed in, 3 refused */
static char login_error[256];
static int login_result; /* rcheevos' code for the last sign-in: RC_OK, RC_NO_RESPONSE, ... */
static int game_state;  /* 0 none, 1 under way, 2 loaded, 3 refused */
static char game_error[256];
static int game_result;

static void keep_error(char* into, const char* error) {
  snprintf(into, 256, "%s", error ? error : "");
}

static uint32_t RC_CCONV read_memory(uint32_t address, uint8_t* buffer, uint32_t num_bytes,
                                     rc_client_t* c) {
  (void)c;
  if (!regions_ready)
    return 0;
  return rc_libretro_memory_read(&regions, address, buffer, num_bytes);
}

static void RC_CCONV server_call(const rc_api_request_t* request,
                                 rc_client_server_callback_t callback, void* data,
                                 rc_client_t* c) {
  (void)c;
  for (int i = 0; i < MAX_PENDING; i++) {
    if (pending[i].callback)
      continue;
    pending[i].id = next_id++;
    pending[i].callback = callback;
    pending[i].data = data;
    omega_cheevos_server_call(pending[i].id, request->url, request->post_data,
                              request->content_type);
    return;
  }
  /* every slot is waiting on the network: let the client try again later */
  rc_api_server_response_t busy;
  memset(&busy, 0, sizeof(busy));
  busy.http_status_code = RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR;
  callback(&busy, data);
}

void omega_cheevos_log(const char* message);

static void RC_CCONV log_message(const char* message, const rc_client_t* c) {
  (void)c;
  omega_cheevos_log(message);
}

static void RC_CCONV event_handler(const rc_client_event_t* event, rc_client_t* c) {
  switch (event->type) {
  case RC_CLIENT_EVENT_ACHIEVEMENT_TRIGGERED:
    omega_cheevos_event(event->type, event->achievement->title, event->achievement->description,
                        event->achievement->points);
    break;
  case RC_CLIENT_EVENT_LEADERBOARD_STARTED:
  case RC_CLIENT_EVENT_LEADERBOARD_FAILED:
  case RC_CLIENT_EVENT_LEADERBOARD_SUBMITTED:
    omega_cheevos_event(event->type, event->leaderboard->title, event->leaderboard->tracker_value,
                        0);
    break;
  case RC_CLIENT_EVENT_GAME_COMPLETED: {
    const rc_client_game_t* game = rc_client_get_game_info(c);
    omega_cheevos_event(event->type, game ? game->title : "", "", 0);
    break;
  }
  case RC_CLIENT_EVENT_SERVER_ERROR:
    omega_cheevos_event(event->type, event->server_error->api, event->server_error->error_message,
                        0);
    break;
  case RC_CLIENT_EVENT_DISCONNECTED:
  case RC_CLIENT_EVENT_RECONNECTED:
    omega_cheevos_event(event->type, "", "", 0);
    break;
  case RC_CLIENT_EVENT_RESET:
    /* hardcore switched on with a game loaded: omega-zero only switches it between games */
    rc_client_reset(c);
    break;
  default:
    break;
  }
}

static void RC_CCONV login_done(int result, const char* error, rc_client_t* c, void* userdata) {
  (void)c;
  (void)userdata;
  login_state = result == RC_OK ? 2 : 3;
  login_result = result;
  keep_error(login_error, error);
}

static void RC_CCONV game_done(int result, const char* error, rc_client_t* c, void* userdata) {
  (void)c;
  (void)userdata;
  /* a load another took the place of, or the game was put away during: that one says */
  if (result == RC_ABORTED)
    return;
  game_state = result == RC_OK ? 2 : 3;
  game_result = result;
  keep_error(game_error, error);
}

static void RC_CCONV core_memory(uint32_t id, rc_libretro_core_memory_info_t* info) {
  info->data = NULL;
  info->size = 0;
  switch (id) {
  case RETRO_MEMORY_SYSTEM_RAM:
    info->data = system_ram;
    info->size = system_len;
    break;
  case RETRO_MEMORY_SAVE_RAM:
    info->data = save_ram;
    info->size = save_len;
    break;
  case RETRO_MEMORY_VIDEO_RAM:
    info->data = video_ram;
    info->size = video_len;
    break;
  default:
    break;
  }
}

int oc_start(int hardcore) {
  if (!client) {
    client = rc_client_create(read_memory, server_call);
    if (!client)
      return 0;
    rc_client_enable_logging(client, RC_CLIENT_LOG_LEVEL_INFO, log_message);
    rc_client_set_event_handler(client, event_handler);
    /* memory is read only in oc_frame, on the thread that runs the core */
    rc_client_set_allow_background_memory_reads(client, 0);
    use_disc_readers();
  }
  rc_client_set_hardcore_enabled(client, hardcore);
  return 1;
}

int oc_hash(uint32_t console, const char* path, char* out) {
  use_disc_readers();
  return rc_hash_generate_from_file(out, console, path);
}

int oc_hardcore(void) {
  return client && rc_client_get_hardcore_enabled(client) && rc_client_is_game_loaded(client);
}

size_t oc_user_agent(char* buffer, size_t size) {
  return client ? rc_client_get_user_agent_clause(client, buffer, size) : 0;
}

void oc_login_password(const char* user, const char* password) {
  login_state = 1;
  login_error[0] = '\0';
  rc_client_begin_login_with_password(client, user, password, login_done, NULL);
}

void oc_login_token(const char* user, const char* token) {
  login_state = 1;
  login_error[0] = '\0';
  rc_client_begin_login_with_token(client, user, token, login_done, NULL);
}

int oc_login_state(const char** error) {
  *error = login_error;
  return login_state;
}

/* Whether the last refusal was the network's, not the account's: the server never answered,
   or answered with something that is not its JSON, as a captive portal does. */
static int unreachable(int result) {
  return result == RC_NO_RESPONSE || result == RC_INVALID_JSON;
}

int oc_login_unreachable(void) {
  return login_state == 3 && unreachable(login_result);
}

int oc_game_unreachable(void) {
  return game_state == 3 && unreachable(game_result);
}

/* The memory of a game is set, so its achievements can be loaded again. */
int oc_has_memory(void) {
  return regions_ready;
}

void oc_user_scores(uint32_t* hardcore, uint32_t* softcore) {
  const rc_client_user_t* user = client ? rc_client_get_user_info(client) : NULL;
  *hardcore = user ? user->score : 0;
  *softcore = user ? user->score_softcore : 0;
}

/* An unlock earned offline, as the request that sends it now: `seconds` after it was earned,
   signed as the site checks it. Writes the post data into `out`; returns its length, or 0. */
size_t oc_award_post(uint32_t id, const char* user, const char* token, int hardcore,
                     const char* hash, uint32_t seconds, char* out, size_t size) {
  rc_api_award_achievement_request_t params;
  rc_api_request_t request;
  size_t n = 0;
  memset(&params, 0, sizeof(params));
  memset(&request, 0, sizeof(request));
  params.username = user;
  params.api_token = token;
  params.achievement_id = id;
  params.hardcore = hardcore ? 1 : 0;
  params.game_hash = hash;
  params.seconds_since_unlock = seconds;
  if (rc_api_init_award_achievement_request(&request, &params) == RC_OK && request.post_data) {
    n = strlen(request.post_data);
    if (n < size)
      memcpy(out, request.post_data, n + 1);
    else
      n = 0;
  }
  rc_api_destroy_request(&request);
  return n;
}

const char* oc_user_name(void) {
  const rc_client_user_t* user = client ? rc_client_get_user_info(client) : NULL;
  return user ? user->display_name : NULL;
}

const char* oc_user_token(void) {
  const rc_client_user_t* user = client ? rc_client_get_user_info(client) : NULL;
  return user ? user->token : NULL;
}

void oc_logout(void) {
  if (client)
    rc_client_logout(client);
  login_state = 0;
  /* signing out lets go of the game too */
  game_state = 0;
}

void oc_set_memory(const struct retro_memory_descriptor* descriptors, unsigned count,
                   uint32_t console, uint8_t* system, size_t system_size, uint8_t* save,
                   size_t save_size, uint8_t* video, size_t video_size) {
  struct retro_memory_map map;
  map.descriptors = descriptors;
  map.num_descriptors = count;
  system_ram = system;
  system_len = system_size;
  save_ram = save;
  save_len = save_size;
  video_ram = video;
  video_len = video_size;
  if (regions_ready)
    rc_libretro_memory_destroy(&regions);
  regions_ready = rc_libretro_memory_init(&regions, count ? &map : NULL, core_memory, console);
}

void oc_load_game(uint32_t console, const char* path) {
  game_state = 1;
  game_error[0] = '\0';
  rc_client_begin_identify_and_load_game(client, console, path, NULL, 0, game_done, NULL);
}

/* The game by the hash made of it beforehand, off the client's lock: hashing a PSP game's
   disc takes a third of a second, which every other call into the client would wait out. */
/* A game is being hashed, on a thread of its own: its achievements are on their way. */
void oc_game_hashing(void) {
  game_state = 1;
  game_error[0] = ' ';
}

void oc_load_game_hash(const char* hash) {
  game_state = 1;
  game_error[0] = '\0';
  rc_client_begin_load_game(client, hash, game_done, NULL);
}

int oc_game_state(const char** error) {
  *error = game_error;
  return game_state;
}

void oc_unload_game(void) {
  if (client)
    rc_client_unload_game(client);
  if (regions_ready)
    rc_libretro_memory_destroy(&regions);
  regions_ready = 0;
  game_state = 0;
}

int oc_summary(const char** title, uint32_t* id, uint32_t* unlocked, uint32_t* total,
               uint32_t* points_unlocked, uint32_t* points_total) {
  if (!client || !rc_client_is_game_loaded(client))
    return 0;
  rc_client_user_game_summary_t summary;
  rc_client_get_user_game_summary(client, &summary);
  const rc_client_game_t* game = rc_client_get_game_info(client);
  *title = game ? game->title : "";
  *id = game ? game->id : 0;
  *unlocked = summary.num_unlocked_achievements;
  *total = summary.num_core_achievements;
  *points_unlocked = summary.points_unlocked;
  *points_total = summary.points_core;
  return 1;
}

/* Hands each of the game's achievements to `each`, in the order the client groups them: the
   ones close to being earned and the ones just earned first, then the rest. */
typedef void (*oc_achievement_fn)(void* user, const char* title, const char* description,
                                  uint32_t points, int unlocked, const char* progress);

void oc_achievements(oc_achievement_fn each, void* user) {
  if (!client || !rc_client_is_game_loaded(client))
    return;
  rc_client_achievement_list_t* list = rc_client_create_achievement_list(
      client, RC_CLIENT_ACHIEVEMENT_CATEGORY_CORE, RC_CLIENT_ACHIEVEMENT_LIST_GROUPING_LOCK_STATE);
  if (!list)
    return;
  for (uint32_t b = 0; b < list->num_buckets; b++) {
    const rc_client_achievement_bucket_t* bucket = &list->buckets[b];
    for (uint32_t i = 0; i < bucket->num_achievements; i++) {
      const rc_client_achievement_t* a = bucket->achievements[i];
      each(user, a->title, a->description, a->points,
           a->state == RC_CLIENT_ACHIEVEMENT_STATE_UNLOCKED, a->measured_progress);
    }
  }
  rc_client_destroy_achievement_list(list);
}

void oc_frame(void) {
  if (client && regions_ready)
    rc_client_do_frame(client);
}

void oc_idle(void) {
  if (client)
    rc_client_idle(client);
}

void oc_reset(void) {
  if (client && rc_client_is_game_loaded(client))
    rc_client_reset(client);
}

void oc_answer(uint32_t id, int status, const char* body, size_t length) {
  for (int i = 0; i < MAX_PENDING; i++) {
    if (!pending[i].callback || pending[i].id != id)
      continue;
    rc_client_server_callback_t callback = pending[i].callback;
    void* data = pending[i].data;
    pending[i].callback = NULL;
    rc_api_server_response_t response;
    memset(&response, 0, sizeof(response));
    response.body = body;
    response.body_length = length;
    response.http_status_code = status;
    callback(&response, data);
    return;
  }
}
