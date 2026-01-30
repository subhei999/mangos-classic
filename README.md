# Slamrock Hardcore (CMaNGOS Classic) — Fork README
[![Windows](../../actions/workflows/windows.yml/badge.svg)](../../actions/workflows/windows.yml) [![Ubuntu](../../actions/workflows/ubuntu.yml/badge.svg)](../../actions/workflows/ubuntu.yml) [![MacOS](../../actions/workflows/macos.yml/badge.svg)](../../actions/workflows/macos.yml)

This file is part of the CMaNGOS Project. See [AUTHORS](AUTHORS.md) and [COPYRIGHT](COPYRIGHT.md) files for Copyright information

## This fork: Slamrock Hardcore PvPvE

This fork’s goal is to build a **World of Warcraft (1.12.1) Hardcore-style mode** with **high replayability** that can be enjoyed with a **small group of friends**, and that **incorporates and encourages PvPvE** through **playerbots**.

### Design pillars

* **Hardcore stakes**: death matters (progress and gear risk) without removing all recovery paths.
* **Replayability**: unpredictable encounters, meaningful loot outcomes, and emergent PvP.
* **PvPvE with playerbots**: the world is dangerous even without a large player population.

### Current gameplay modifications (high level)

1. **Open-world FFA PvP**: all zones outside capital cities and starting areas are treated as FFA arena.
2. **Anti-grief level band**: you can only initiate attacks within a **5 level** difference.
   * Higher levels cannot engage targets more than 5 levels below them.
   * If the lower-level player initiates combat, the higher-level player may fight back.
3. **Playerbot aggression**:
   * Playerbots have a chance to be aggressive and attack first.
   * All playerbots will defend themselves.
   * Playerbot vs playerbot combat requires **both** bots to be in an aggressive state.
4. **Death consequences / recovery**:
   * Players drop **all equipped items** and **half their money** into a lootable bag (lootable by players or playerbots).
   * Players also lose XP on death; you can recover **half** the lost XP if you reclaim your body (i.e. not spirit rez).
   * Playerbots also drop gear, but at a **low chance** (typically 1–3 items).
5. **Slamrock item (gear gambling)**:
   * Added the item **Slamrock** which allows players to **SLAM** an item.
   * Outcomes:
     * **1–3 random enchantments** appropriate for the item’s ilevel, **or**
     * **25%** chance to **brick** the item, **or**
     * **2%** chance to **upgrade** it by up to **+5 ilevel** (same gear type).
6. **Convenience addons (separate repo)**:
   * Click-to-teleport on the map (useful when playing with GM powers).
   * BotMap overlay showing playerbot locations on the world map.

## Welcome to C(ontinued)-MaNGOS

CMaNGOS is a free project with the following goal:

  **Doing Emulation Right!**

This means, we want to focus on:

* Doing
  * This project is focused on developing software!
  * Also there are many other aspects that need to be done and are
    considered equally important.
  * Anyone who wants to do stuff is very welcome to do so!

* Emulation
  * This project is about developing a server software that is able to
    emulate a well known MMORPG service.

* Right
  * Our goal must always be to provide the best code that we can.
  * Being 'right' is defined by the behaviour of the system
    we want to emulate.
  * Developing things right also includes documenting and discussing
    _how_ to do things better, hence...
  * Learning and teaching are very important in our view, and must
    always be a part of what we do.

To be able to accomplish these goals, we support and promote:

* Freedom
  * of our work: Our work - including our code - is released under the GPL.
    So everybody is free to use and contribute to this open source project.
  * for our developers and contributors on things that interest them.
    No one here is telling anybody _what_ to do.
    If you want somebody to do something for you, pay them,
    but we are here to enjoy.
  * to have FUN with developing.

* A friendly environment
  * We try to leave personal issues behind us.
  * We only argue about content and not about thin air!
  * We follow the [Netiquette](http://tools.ietf.org/html/rfc1855).

-- The C(ontinued)-MaNGOS Team!

## Further information

  You can find further information about CMaNGOS at the following places:
  * [CMaNGOS Discord](https://discord.gg/Dgzerzb)
  * [GitHub repositories](https://github.com/cmangos/)
  * [Issue tracker](https://github.com/cmangos/issues/issues)
  * [Pull Requests](https://github.com/cmangos/mangos-classic/pulls)
  * [Wiki](https://github.com/cmangos/issues/wiki) with additional information on installation
  * [Contributing Guidelines](CONTRIBUTING.md)
  * Documentation can be found in the doc/ subdirectory and on the GitHub wiki

## Related repositories (forks)

  This core repository is typically used together with:

  * [classic-db (world database)](https://github.com/subhei999/classic-db)
  * [playerbots (bot AI)](https://github.com/subhei999/playerbots)
  * [SubsAddons (convenience addons)](https://github.com/subhei999/SubsAddons)

  This fork of the core:

  * [mangos-classic (core)](https://github.com/subhei999/mangos-classic)

## License

  CMaNGOS is free software; you can redistribute it and/or modify
  it under the terms of the GNU General Public License as published by
  the Free Software Foundation; either version 2 of the License, or
  (at your option) any later version.

  This program is distributed in the hope that it will be useful,
  but WITHOUT ANY WARRANTY; without even the implied warranty of
  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
  GNU General Public License for more details.

  You should have received a copy of the GNU General Public License
  along with this program; if not, write to the Free Software
  Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307  USA


  You can find the full license text in the file [COPYING](COPYING) delivered with this package.

### Exceptions to GPL

  World of Warcraft® ©2004 Blizzard Entertainment, Inc. All rights reserved.
  World of Warcraft® content and materials mentioned or referenced are copyrighted by
  Blizzard Entertainment, Inc. or its licensors.
  World of Warcraft, WoW, Warcraft, The Frozen Throne, The Burning Crusade, Wrath of the Lich King,
  Cataclysm, Mists of Pandaria, Ashbringer, Dark Portal, Darkmoon Faire, Frostmourne, Onyxia's Lair,
  Diablo, Hearthstone, Heroes of Azeroth, Reaper of Souls, Starcraft, Battle Net, Blizzcon, Glider,
  Blizzard and Blizzard Entertainment are trademarks or registered trademarks of
  Blizzard Entertainment, Inc. in the U.S. and/or other countries.

  Any World of Warcraft® content and materials mentioned or referenced are copyrighted by
  Blizzard Entertainment, Inc. or its licensors.
  CMaNGOS project is not affiliated with Blizzard Entertainment, Inc. or its licensors.

  Some third-party libraries CMaNGOS uses have other licenses, that must be
  upheld.  These libraries are located within the dep/ directory

  In addition, as a special exception, the CMaNGOS project
  gives permission to link the code of its release of MaNGOS with the
  OpenSSL project's "OpenSSL" library (or with modified versions of it
  that use the same license as the "OpenSSL" library), and distribute
  the linked executables.  You must obey the GNU General Public License
  in all respects for all of the code used other than "OpenSSL".  If you
  modify this file, you may extend this exception to your version of the
  file, but you are not obligated to do so.  If you do not wish to do
  so, delete this exception statement from your version.
