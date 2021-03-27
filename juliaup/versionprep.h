#pragma once

#include "version.h"

#define STRINGIZE2(s) #s
#define STRINGIZE(s) STRINGIZE2(s)
#define JULIA_APP_VERSION            JULIA_APP_VERSION_MAJOR, JULIA_APP_VERSION_MINOR, JULIA_APP_VERSION_REVISION, JULIA_APP_VERSION_BUILD
#define JULIA_APP_VERSION_STR        STRINGIZE(JULIA_APP_VERSION_MAJOR)        \
                                    "." STRINGIZE(JULIA_APP_VERSION_MINOR)    \
                                    "." STRINGIZE(JULIA_APP_VERSION_REVISION) \
                                    "." STRINGIZE(JULIA_APP_VERSION_BUILD)    \

