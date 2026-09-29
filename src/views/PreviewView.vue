<script setup>
import {useRoute} from "vue-router";
import {onMounted, reactive} from "vue";
import {invoke} from "@tauri-apps/api/core";
import {naturalSort} from '../utils';

const route = useRoute();
let zip_path = route?.query?.path;
const preview_list = reactive([]);

// 自然排序函数
onMounted(async () => {
  preview_list.length = 0;
  const entries = (await invoke('read_zip_entries', {path: zip_path})).sort(naturalSort);
  for (const entry of entries) {
    if (!entry.directory) {
      const imageData = await invoke('read_zip_file', {path: zip_path, filename: entry.filename});
      const imageBlob = new Blob([new Uint8Array(imageData)]);
      let imageURL = URL.createObjectURL(imageBlob);
      preview_list.push({preview: imageURL, filename: entry.filename});
    }
  }
});
</script>

<template>
  <ul class="w-full">
    <li v-for="item in preview_list" :key="item.filename" class="w-full">
      <img class="w-full object-contain" :src="item.preview" :alt="item.filename">
    </li>
  </ul>
</template>

<style scoped>

</style>
