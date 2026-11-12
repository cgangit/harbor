#ifndef HARBOR_STARFISH_H
#define HARBOR_STARFISH_H

#include <stdint.h>

int sf_create(void);
int sf_load(const char *payload);
int sf_play(void);
int sf_pause(void);
int sf_feed(const void *bytes, int len, int64_t pts_ns, int es);
int sf_flush(void);
int sf_unload(void);
void sf_destroy(void);

int acb_create(void);
int acb_bind(const char *media_id);
int acb_send_video_data(const void *info, int len);
int acb_start(int x, int y, int w, int h);
int acb_unload(void);

void sf_on_event(int type, const char *json);
void acb_on_event(int type, const char *json);

#endif
