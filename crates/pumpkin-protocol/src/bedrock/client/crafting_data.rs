use crate::{
    bedrock::network_item::NetworkItemDescriptor,
    codec::{var_int::VarInt, var_uint::VarUInt},
    serial::PacketWrite,
};
use pumpkin_macros::packet;
use std::io::{Error, Write};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeItemDescriptor {
    Empty,
    Item {
        identifier: String,
        metadata_value: i32,
    },
    Tag(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemDescriptorCount {
    pub descriptor: RecipeItemDescriptor,
    pub count: i32,
}

impl ItemDescriptorCount {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            descriptor: RecipeItemDescriptor::Empty,
            count: 0,
        }
    }

    #[must_use]
    pub const fn item(identifier: String, metadata_value: i32) -> Self {
        Self {
            descriptor: RecipeItemDescriptor::Item {
                identifier,
                metadata_value,
            },
            count: 1,
        }
    }

    #[must_use]
    pub const fn tag(identifier: String) -> Self {
        Self {
            descriptor: RecipeItemDescriptor::Tag(identifier),
            count: 1,
        }
    }
}

impl PacketWrite for ItemDescriptorCount {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        match &self.descriptor {
            RecipeItemDescriptor::Empty => {
                VarUInt(0).write(writer)?;
                VarInt(32767).write(writer)?;
            }
            RecipeItemDescriptor::Item {
                identifier,
                metadata_value,
            } => {
                VarUInt(1).write(writer)?;
                "name".write(writer)?;
                identifier.write(writer)?;
                VarInt(*metadata_value).write(writer)?;
            }
            RecipeItemDescriptor::Tag(identifier) => {
                VarUInt(1).write(writer)?;
                "item_tag".write(writer)?;
                identifier.write(writer)?;
                VarInt(32767).write(writer)?;
            }
        }
        VarInt(self.count).write(writer)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RecipeUnlockRequirement {
    pub context: i32,
}

impl PacketWrite for RecipeUnlockRequirement {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        VarInt(self.context).write(writer)?;
        // Context NONE carries an optional ingredient list; Pumpkin currently
        // sends ALWAYS for its generated recipes.
        false.write(writer)
    }
}

#[derive(Clone, Debug)]
pub struct BedrockShapelessRecipe {
    pub recipe_id: String,
    pub input: Vec<ItemDescriptorCount>,
    pub output: Vec<NetworkItemDescriptor>,
    pub uuid: Uuid,
    pub block: String,
    pub priority: VarInt,
    pub unlock_requirement: RecipeUnlockRequirement,
    pub recipe_network_id: VarUInt,
}

impl PacketWrite for BedrockShapelessRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.recipe_id.write(writer)?;

        // input slice with VarUInt length prefix
        VarUInt(self.input.len() as u32).write(writer)?;
        for item in &self.input {
            item.write(writer)?;
        }

        // output slice with VarUInt length prefix
        VarUInt(self.output.len() as u32).write(writer)?;
        for item in &self.output {
            item.write_item_instance(writer)?;
        }

        // uuid
        self.uuid.write(writer)?;

        // block
        self.block.write(writer)?;

        // priority
        self.priority.write(writer)?;

        true.write(writer)?;
        self.unlock_requirement.write(writer)?;

        // recipe_network_id
        self.recipe_network_id.write(writer)?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BedrockShapedRecipe {
    pub recipe_id: String,
    pub width: VarInt,
    pub height: VarInt,
    pub input: Vec<ItemDescriptorCount>,
    pub output: Vec<NetworkItemDescriptor>,
    pub uuid: Uuid,
    pub block: String,
    pub priority: VarInt,
    pub assume_symmetry: bool,
    pub unlock_requirement: RecipeUnlockRequirement,
    pub recipe_network_id: VarUInt,
}

impl PacketWrite for BedrockShapedRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.recipe_id.write(writer)?;
        self.width.write(writer)?;
        self.height.write(writer)?;

        VarUInt(self.input.len() as u32).write(writer)?;
        for item in &self.input {
            item.write(writer)?;
        }

        // output slice with VarUInt length prefix
        VarUInt(self.output.len() as u32).write(writer)?;
        for item in &self.output {
            item.write_item_instance(writer)?;
        }

        // uuid
        self.uuid.write(writer)?;

        // block
        self.block.write(writer)?;

        // priority
        self.priority.write(writer)?;

        // assume_symmetry
        self.assume_symmetry.write(writer)?;

        true.write(writer)?;
        self.unlock_requirement.write(writer)?;

        // recipe_network_id
        self.recipe_network_id.write(writer)?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BedrockFurnaceRecipe {
    pub recipe_id: String,
    pub input: ItemDescriptorCount,
    pub output: NetworkItemDescriptor,
    pub block: String,
    pub recipe_network_id: VarUInt,
}

impl PacketWrite for BedrockFurnaceRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.recipe_id.write(writer)?;
        self.input.write(writer)?;
        self.output.write_item_instance(writer)?;
        self.block.write(writer)?;
        self.recipe_network_id.write(writer)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BedrockSmithingTransformRecipe {
    pub recipe_id: String,
    pub template: ItemDescriptorCount,
    pub base: ItemDescriptorCount,
    pub addition: ItemDescriptorCount,
    pub result: NetworkItemDescriptor,
    pub block: String,
    pub recipe_network_id: VarUInt,
}

impl PacketWrite for BedrockSmithingTransformRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.recipe_id.write(writer)?;
        self.template.write(writer)?;
        self.base.write(writer)?;
        self.addition.write(writer)?;
        self.result.write_item_instance(writer)?;
        self.block.write(writer)?;
        self.recipe_network_id.write(writer)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BedrockSmithingTrimRecipe {
    pub recipe_id: String,
    pub template: ItemDescriptorCount,
    pub base: ItemDescriptorCount,
    pub addition: ItemDescriptorCount,
    pub block: String,
    pub recipe_network_id: VarUInt,
}

impl PacketWrite for BedrockSmithingTrimRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.recipe_id.write(writer)?;
        self.template.write(writer)?;
        self.base.write(writer)?;
        self.addition.write(writer)?;
        self.block.write(writer)?;
        self.recipe_network_id.write(writer)?;
        Ok(())
    }
}

/// Bedrock potion mix (brewing) recipe: input potion item id + meta, reagent
/// item id + meta and output potion item id + meta.
#[derive(Clone, Debug)]
pub struct PotionMixRecipe {
    pub input_id: VarInt,
    pub input_meta: VarInt,
    pub reagent_id: VarInt,
    pub reagent_meta: VarInt,
    pub output_id: VarInt,
    pub output_meta: VarInt,
}

impl PacketWrite for PotionMixRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.input_id.write(writer)?;
        self.input_meta.write(writer)?;
        self.reagent_id.write(writer)?;
        self.reagent_meta.write(writer)?;
        self.output_id.write(writer)?;
        self.output_meta.write(writer)?;
        Ok(())
    }
}

/// Bedrock container mix recipe (e.g. drinkable potion + gunpowder -> splash).
#[derive(Clone, Debug)]
pub struct ContainerMixRecipe {
    pub input_id: VarInt,
    pub reagent_id: VarInt,
    pub output_id: VarInt,
}

impl PacketWrite for ContainerMixRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.input_id.write(writer)?;
        self.reagent_id.write(writer)?;
        self.output_id.write(writer)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum BedrockRecipe {
    Shapeless(BedrockShapelessRecipe),
    Shaped(BedrockShapedRecipe),
    Furnace(BedrockFurnaceRecipe),
    SmithingTransform(BedrockSmithingTransformRecipe),
    SmithingTrim(BedrockSmithingTrimRecipe),
}

impl PacketWrite for BedrockRecipe {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            Self::Shapeless(recipe) => {
                VarInt(0).write(writer)?; // type 0: Shapeless
                recipe.write(writer)?;
            }
            Self::Shaped(recipe) => {
                VarInt(1).write(writer)?; // type 1: Shaped
                recipe.write(writer)?;
            }
            Self::Furnace(recipe) => {
                VarInt(2).write(writer)?; // type 2: Furnace
                recipe.write(writer)?;
            }
            Self::SmithingTransform(recipe) => {
                VarInt(8).write(writer)?; // type 8: SmithingTransform
                recipe.write(writer)?;
            }
            Self::SmithingTrim(recipe) => {
                VarInt(9).write(writer)?; // type 9: SmithingTrim
                recipe.write(writer)?;
            }
        }
        Ok(())
    }
}

#[packet(52)]
pub struct CCraftingData {
    pub recipes: Vec<BedrockRecipe>,
    pub potion_mixes: Vec<PotionMixRecipe>,
    pub container_mixes: Vec<ContainerMixRecipe>,
    pub clean_recipes: bool,
}

impl PacketWrite for CCraftingData {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        // The 1.26.40 packet carries one array per recipe type, written without
        // the per-entry type prefix.
        let shaped = self
            .recipes
            .iter()
            .filter_map(|recipe| match recipe {
                BedrockRecipe::Shaped(recipe) => Some(recipe),
                _ => None,
            })
            .collect::<Vec<_>>();
        VarUInt(shaped.len() as u32).write(writer)?;
        for recipe in shaped {
            recipe.write(writer)?;
        }

        let shapeless = self
            .recipes
            .iter()
            .filter_map(|recipe| match recipe {
                BedrockRecipe::Shapeless(recipe) => Some(recipe),
                _ => None,
            })
            .collect::<Vec<_>>();
        VarUInt(shapeless.len() as u32).write(writer)?;
        for recipe in shapeless {
            recipe.write(writer)?;
        }

        // Furnace recipes have no array in 1.26.40; the client knows smelting
        // from its own data, so BedrockRecipe::Furnace entries go nowhere here.

        // Multi, user-data shapeless and chemistry recipe arrays.
        for _ in 0..4 {
            VarUInt(0).write(writer)?;
        }

        let smithing_transforms = self
            .recipes
            .iter()
            .filter_map(|recipe| match recipe {
                BedrockRecipe::SmithingTransform(recipe) => Some(recipe),
                _ => None,
            })
            .collect::<Vec<_>>();
        VarUInt(smithing_transforms.len() as u32).write(writer)?;
        for recipe in smithing_transforms {
            recipe.write(writer)?;
        }

        let smithing_trims = self
            .recipes
            .iter()
            .filter_map(|recipe| match recipe {
                BedrockRecipe::SmithingTrim(recipe) => Some(recipe),
                _ => None,
            })
            .collect::<Vec<_>>();
        VarUInt(smithing_trims.len() as u32).write(writer)?;
        for recipe in smithing_trims {
            recipe.write(writer)?;
        }

        VarUInt(self.potion_mixes.len() as u32).write(writer)?;
        for recipe in &self.potion_mixes {
            recipe.write(writer)?;
        }

        VarUInt(self.container_mixes.len() as u32).write(writer)?;
        for recipe in &self.container_mixes {
            recipe.write(writer)?;
        }

        // Material reducer array.
        VarUInt(0).write(writer)?;

        // clean_recipes
        self.clean_recipes.write(writer)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::serial::PacketWrite;

    use super::ItemDescriptorCount;

    #[test]
    fn item_tag_descriptor_uses_bedrock_recipe_wire_format() {
        let mut encoded = Vec::new();
        ItemDescriptorCount::tag("minecraft:planks".to_string())
            .write(&mut encoded)
            .unwrap();

        let mut expected = vec![1, 8];
        expected.extend_from_slice(b"item_tag");
        expected.push(16);
        expected.extend_from_slice(b"minecraft:planks");
        expected.extend_from_slice(&[0xfe, 0xff, 0x03, 0x02]);
        assert_eq!(encoded, expected);
    }
}
