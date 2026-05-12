#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum SpellCastSource {
    Player,
    Item { item_guid: ObjectGuid },
    GameObject { gameobject_guid: ObjectGuid },
    Triggered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum SpellLifecycleState {
    Created,
    Preparing,
    Casting,
    Traveling,
    Channeling,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreparedSpellCast {
    spell_id: u32,
    source: SpellCastSource,
    state: SpellLifecycleState,
    profile: SpellCastProfile,
}

impl PreparedSpellCast {
    fn new(spell_id: u32, source: SpellCastSource, profile: SpellCastProfile) -> Self {
        Self {
            spell_id,
            source,
            state: SpellLifecycleState::Created,
            profile,
        }
    }

    fn prepare(&mut self) {
        self.state = SpellLifecycleState::Preparing;
    }

    fn start_casting(&mut self) {
        self.state = SpellLifecycleState::Casting;
    }

    fn finish(&mut self) {
        self.state = SpellLifecycleState::Finished;
    }

    fn spell_start_body(
        &mut self,
        caster: ObjectGuid,
        cast_time_ms: u32,
        targets: &SpellCastTargets,
    ) -> anyhow::Result<Vec<u8>> {
        self.start_casting();
        let source = self.packet_source(caster);
        build_spell_start_body_with_source(source, caster, self.spell_id, cast_time_ms, targets)
    }

    fn spell_go_body(
        &mut self,
        caster: ObjectGuid,
        targets: &SpellCastTargets,
    ) -> anyhow::Result<Vec<u8>> {
        let source = self.packet_source(caster);
        build_spell_go_body_with_source(
            source,
            caster,
            self.spell_id,
            self.go_cast_flags(),
            targets,
            None,
        )
    }

    fn packet_source(&self, caster: ObjectGuid) -> ObjectGuid {
        match self.source {
            SpellCastSource::Player | SpellCastSource::Triggered => caster,
            SpellCastSource::Item { item_guid } => item_guid,
            SpellCastSource::GameObject { gameobject_guid } => gameobject_guid,
        }
    }

    fn go_cast_flags(&self) -> u16 {
        match self.source {
            SpellCastSource::Item { .. } => CAST_FLAG_SPELL_GO | CAST_FLAG_ITEM_CASTER,
            _ => CAST_FLAG_SPELL_GO,
        }
    }
}
