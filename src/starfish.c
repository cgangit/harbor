#include "starfish.h"

#include <dlfcn.h>
#include <stddef.h>
#include <stdint.h>

static void *g_player;
static void *g_acb;

static int (*p_load)(const char *);
static int (*p_play)(void);
static int (*p_pause)(void);
static int (*p_feed)(const void *, int, int64_t, int);
static int (*p_flush)(void);
static int (*p_unload)(void);
static void (*p_destroy)(void);
static int (*p_acb_create)(void);
static int (*p_acb_bind)(const char *);
static int (*p_acb_send)(const void *, int);
static int (*p_acb_start)(int, int, int, int);
static int (*p_acb_unload)(void);

static void *first_sym(void *lib, const char *const *names)
{
    size_t i;
    for (i = 0; names[i] != NULL; i++) {
        void *s = dlsym(lib, names[i]);
        if (s != NULL) {
            return s;
        }
    }
    return NULL;
}

int sf_create(void)
{
    static const char *feed_names[] = {
        "_ZN8Starfish4FeedEPKvix",
        "_ZN8Starfish4FeedEPKcix",
        "Feed",
        NULL
    };
    if (g_player != NULL) {
        return 0;
    }
    g_player = dlopen("libplayerAPIs.so", RTLD_NOW);
    if (g_player == NULL) {
        return -1;
    }
    p_feed = (int (*)(const void *, int, int64_t, int))first_sym(g_player, feed_names);
    p_load = (int (*)(const char *))dlsym(g_player, "Load");
    p_play = (int (*)(void))dlsym(g_player, "Play");
    p_pause = (int (*)(void))dlsym(g_player, "Pause");
    p_flush = (int (*)(void))dlsym(g_player, "Flush");
    p_unload = (int (*)(void))dlsym(g_player, "Unload");
    p_destroy = (void (*)(void))dlsym(g_player, "Destroy");
    if (p_feed == NULL) {
        return -1;
    }
    return 0;
}

int sf_load(const char *payload)
{
    if (p_load == NULL) {
        return -1;
    }
    return p_load(payload);
}

int sf_play(void)
{
    if (p_play == NULL) {
        return -1;
    }
    return p_play();
}

int sf_pause(void)
{
    if (p_pause == NULL) {
        return -1;
    }
    return p_pause();
}

int sf_feed(const void *bytes, int len, int64_t pts_ns, int es)
{
    if (p_feed == NULL) {
        return -1;
    }
    return p_feed(bytes, len, pts_ns, es);
}

int sf_flush(void)
{
    if (p_flush == NULL) {
        return -1;
    }
    return p_flush();
}

int sf_unload(void)
{
    if (p_unload == NULL) {
        return -1;
    }
    return p_unload();
}

void sf_destroy(void)
{
    if (p_destroy != NULL) {
        p_destroy();
    }
    if (g_player != NULL) {
        dlclose(g_player);
        g_player = NULL;
    }
    p_load = NULL;
    p_play = NULL;
    p_pause = NULL;
    p_feed = NULL;
    p_flush = NULL;
    p_unload = NULL;
    p_destroy = NULL;
}

int acb_create(void)
{
    if (g_acb != NULL) {
        return 0;
    }
    g_acb = dlopen("libAcb.so", RTLD_NOW);
    if (g_acb == NULL) {
        return -1;
    }
    p_acb_create = (int (*)(void))dlsym(g_acb, "create");
    p_acb_bind = (int (*)(const char *))dlsym(g_acb, "bind");
    p_acb_send = (int (*)(const void *, int))dlsym(g_acb, "setMediaVideoData");
    p_acb_start = (int (*)(int, int, int, int))dlsym(g_acb, "setDisplayWindow");
    p_acb_unload = (int (*)(void))dlsym(g_acb, "setState");
    if (p_acb_bind == NULL) {
        return -1;
    }
    if (p_acb_create != NULL) {
        return p_acb_create();
    }
    return 0;
}

int acb_bind(const char *media_id)
{
    if (p_acb_bind == NULL) {
        return -1;
    }
    return p_acb_bind(media_id);
}

int acb_send_video_data(const void *info, int len)
{
    if (p_acb_send == NULL) {
        return -1;
    }
    return p_acb_send(info, len);
}

int acb_start(int x, int y, int w, int h)
{
    if (p_acb_start == NULL) {
        return -1;
    }
    return p_acb_start(x, y, w, h);
}

int acb_unload(void)
{
    if (p_acb_unload == NULL) {
        return -1;
    }
    return p_acb_unload();
}


