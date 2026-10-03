use super::{ArrayKind, EmuError, Handle, HashMap, HeapValue, Machine, Value, heap_error};

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_properties_table(
        &mut self,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        enum PropertyValue {
            Boolean(bool),
            Integer(i32),
        }
        let properties = [
            (
                "supportAntialiasing",
                PropertyValue::Boolean(self.limits.m3g_support_antialiasing),
            ),
            (
                "supportTrueColor",
                PropertyValue::Boolean(self.limits.m3g_support_true_color),
            ),
            (
                "supportDithering",
                PropertyValue::Boolean(self.limits.m3g_support_dithering),
            ),
            (
                "supportMipmapping",
                PropertyValue::Boolean(self.limits.m3g_support_mipmapping),
            ),
            (
                "supportPerspectiveCorrection",
                PropertyValue::Boolean(self.limits.m3g_support_perspective_correction),
            ),
            (
                "supportLocalCameraLighting",
                PropertyValue::Boolean(self.limits.m3g_support_local_camera_lighting),
            ),
            (
                "maxLights",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_max_lights).unwrap_or(i32::MAX),
                ),
            ),
            (
                "maxViewportWidth",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_max_viewport_width).unwrap_or(i32::MAX),
                ),
            ),
            (
                "maxViewportHeight",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_max_viewport_height).unwrap_or(i32::MAX),
                ),
            ),
            (
                "maxViewportDimension",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_max_viewport_dimension).unwrap_or(i32::MAX),
                ),
            ),
            (
                "maxTextureDimension",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_max_texture_dimension).unwrap_or(i32::MAX),
                ),
            ),
            (
                "maxSpriteCropDimension",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_max_sprite_crop_dimension).unwrap_or(i32::MAX),
                ),
            ),
            (
                "maxTransformsPerVertex",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_max_transforms_per_vertex).unwrap_or(i32::MAX),
                ),
            ),
            (
                "numTextureUnits",
                PropertyValue::Integer(
                    i32::try_from(self.limits.m3g_num_texture_units).unwrap_or(i32::MAX),
                ),
            ),
        ];
        let length = i32::try_from(properties.len()).unwrap_or(i32::MAX);
        let keys = self.allocate_array(
            ArrayKind::Reference("java/lang/Object".into()),
            length,
            &[],
            roots,
        )?;
        let values = self.allocate_array(
            ArrayKind::Reference("java/lang/Object".into()),
            length,
            &[Some(Value::Reference(Some(keys)))],
            roots,
        )?;
        self.heap.temporary_roots.extend([keys, values]);
        let result = (|| {
            for (index, (name, value)) in properties.into_iter().enumerate() {
                let key = self.intern_string(name, &[], roots)?;
                let (class, field, primitive) = match value {
                    PropertyValue::Boolean(value) => (
                        "java/lang/Boolean",
                        "java/lang/Boolean.value:Z",
                        i32::from(value),
                    ),
                    PropertyValue::Integer(value) => {
                        ("java/lang/Integer", "java/lang/Integer.value:I", value)
                    }
                };
                let wrapper = self.allocate_object(
                    class,
                    HashMap::from([(field.to_owned(), HeapValue::Int(primitive))]),
                    &[],
                    roots,
                )?;
                self.heap
                    .managed
                    .array_set(keys, index as i32, HeapValue::Reference(Some(key)))
                    .map_err(heap_error)?;
                self.heap
                    .managed
                    .array_set(values, index as i32, HeapValue::Reference(Some(wrapper)))
                    .map_err(heap_error)?;
            }
            self.allocate_object(
                "java/util/Hashtable",
                HashMap::from([
                    (
                        "java/util/Hashtable.keys:[Ljava/lang/Object;".to_owned(),
                        HeapValue::Reference(Some(keys)),
                    ),
                    (
                        "java/util/Hashtable.values:[Ljava/lang/Object;".to_owned(),
                        HeapValue::Reference(Some(values)),
                    ),
                    (
                        "java/util/Hashtable.count:I".to_owned(),
                        HeapValue::Int(length),
                    ),
                ]),
                &[],
                roots,
            )
        })();
        self.heap.temporary_roots.pop();
        self.heap.temporary_roots.pop();
        result
    }
}
