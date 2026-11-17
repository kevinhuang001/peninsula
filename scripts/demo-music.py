#!/usr/bin/env python3
"""Local MPRIS fixture for checking the real Linux music integration.

Install dbus-next in a virtualenv, then run this alongside Peninsula on the
same D-Bus session. No audio or network lookup is needed; lyrics use a local
LRC file. The sample pauses at 25s and resumes at 29s; PlayPause also works.
"""
import argparse
import asyncio
from pathlib import Path
import time

from dbus_next import Variant, PropertyAccess
from dbus_next.aio import MessageBus
from dbus_next.service import ServiceInterface, dbus_property, method


class Player(ServiceInterface):
    def __init__(self, offset):
        super().__init__("org.mpris.MediaPlayer2.Player")
        self.started = time.monotonic() - offset
        self.manual_pause = None

    def elapsed(self):
        return time.monotonic() - self.started

    def position(self):
        if self.manual_pause is not None:
            return self.manual_pause
        elapsed = self.elapsed()
        return min(elapsed, 25.0) if elapsed < 29.0 else elapsed - 4.0

    @dbus_property(access=PropertyAccess.READ)
    def PlaybackStatus(self) -> 's':
        paused = self.manual_pause is not None or 25.0 <= self.elapsed() < 29.0
        return "Paused" if paused else "Playing"

    @dbus_property(access=PropertyAccess.READ)
    def Metadata(self) -> 'a{sv}':
        song = Path(__file__).resolve().parent / "fixtures/music-demo.ogg"
        return {
            "mpris:trackid": Variant('o', '/peninsula/demo'),
            "mpris:length": Variant('x', 180_000_000),
            "xesam:title": Variant('s', '完整听见 · Linux 演示'),
            "xesam:artist": Variant('as', ['Peninsula 本地测试播放器']),
            "xesam:album": Variant('s', '原创测试歌词 / MPRIS + LRC'),
            "xesam:url": Variant('s', song.as_uri()),
        }

    @dbus_property(access=PropertyAccess.READ)
    def Position(self) -> 'x':
        return int(self.position() * 1_000_000)

    @dbus_property(access=PropertyAccess.READ)
    def Rate(self) -> 'd':
        return 1.0

    @dbus_property(access=PropertyAccess.READ)
    def CanControl(self) -> 'b':
        return True

    @dbus_property(access=PropertyAccess.READ)
    def CanPause(self) -> 'b':
        return True

    @dbus_property(access=PropertyAccess.READ)
    def CanPlay(self) -> 'b':
        return True

    @dbus_property(access=PropertyAccess.READ)
    def CanGoPrevious(self) -> 'b':
        return True

    @dbus_property(access=PropertyAccess.READ)
    def CanGoNext(self) -> 'b':
        return True

    @method()
    def PlayPause(self):
        if self.manual_pause is None:
            self.manual_pause = self.position()
        else:
            self.started = time.monotonic() - self.manual_pause
            self.manual_pause = None
        self.emit_properties_changed({'PlaybackStatus': self.PlaybackStatus})

    @method()
    def Previous(self):
        self.started = time.monotonic()
        self.manual_pause = None

    @method()
    def Next(self):
        self.started = time.monotonic() - 14.0
        self.manual_pause = None


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--offset', type=float, default=0.0)
    args = parser.parse_args()
    bus = await MessageBus().connect()
    bus.export('/org/mpris/MediaPlayer2', Player(args.offset))
    await bus.request_name('org.mpris.MediaPlayer2.PeninsulaDemo')
    print('MPRIS demo ready; lyrics are local and no audio is played.', flush=True)
    await asyncio.Future()


if __name__ == '__main__':
    asyncio.run(main())
