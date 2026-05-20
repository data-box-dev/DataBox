<script setup lang="ts">
import { ref, watch, h } from 'vue'
import {
  NModal,
  NForm,
  NFormItem,
  NInput,
  NSelect,
  NInputNumber,
  NSwitch,
  NButton,
  NSpin,
  useMessage,
} from 'naive-ui'
import type { FormInst, FormItemRule } from 'naive-ui'
import type { ConnectionConfig, DriverKind } from '@/types/database'
import { tauriCommands } from '@/composables/useTauriCommands'

const props = defineProps<{
  visible: boolean
  editingConfig?: ConnectionConfig | null
}>()

const emit = defineEmits<{
  updateVisible: [value: boolean]
  saved: [config: ConnectionConfig]
}>()

const formRef = ref<FormInst>()
const loading = ref(false)
const testing = ref(false)
const message = useMessage()

const formModel = ref<Partial<ConnectionConfig>>({
  driver: 'Postgres',
  name: '',
  host: 'localhost',
  port: 5432,
  database: '',
  username: '',
  password: '',
  ssl: false,
  options: {},
})

const driverOptions = [
  { label: 'PostgreSQL', value: 'Postgres' },
  { label: 'MySQL', value: 'Mysql' },
  { label: 'SQLite', value: 'Sqlite' },
  { label: 'MongoDB', value: 'Mongo' },
  { label: 'Redis', value: 'Redis' },
]

const portByDriver: Record<DriverKind, number> = {
  Postgres: 5432,
  Mysql: 3306,
  Sqlite: 0,
  Mongo: 27017,
  Redis: 6379,
}

const rules: Record<string, FormItemRule[]> = {
  name: [
    { required: true, message: '请输入连接名称', trigger: 'blur' },
  ],
  driver: [
    { required: true, message: '请选择数据库驱动', trigger: 'change' },
  ],
  host: [
    { required: true, message: '请输入主机地址', trigger: 'blur' },
  ],
}

watch(
  () => props.visible,
  (visible) => {
    if (visible && props.editingConfig) {
      formModel.value = { ...props.editingConfig }
    } else if (visible) {
      formModel.value = {
        driver: 'Postgres',
        name: '',
        host: 'localhost',
        port: 5432,
        database: '',
        username: '',
        password: '',
        ssl: false,
        options: {},
      }
    }
  }
)

watch(
  () => formModel.value.driver,
  (driver) => {
    if (driver) {
      formModel.value.port = portByDriver[driver as DriverKind]
    }
  }
)

async function testConnection(): Promise<void> {
  testing.value = true
  try {
    await formRef.value?.validate()
    await tauriCommands.testConnection(formModel.value as ConnectionConfig)
    message.success('连接成功')
  } catch (e) {
    message.error(`连接失败: ${e}`)
  } finally {
    testing.value = false
  }
}

async function save(): Promise<void> {
  try {
    await formRef.value?.validate()
    loading.value = true
    const config: ConnectionConfig = {
      ...formModel.value,
      id: props.editingConfig?.id || crypto.randomUUID(),
    } as ConnectionConfig
    await emit('saved', config)
    emit('updateVisible', false)
    message.success('连接已保存')
  } catch (e) {
    // validation error handled by naive-ui
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <NModal
    :show="visible"
    @update:show="emit('updateVisible', $event)"
    preset="card"
    :title="editingConfig ? '编辑连接' : '新建连接'"
    style="width: 520px"
  >
    <NForm
      ref="formRef"
      :model="formModel"
      :rules="rules"
      label-placement="left"
      label-width="80"
    >
      <NFormItem label="名称" path="name" required>
        <NInput v-model:value="formModel.name" placeholder="My PostgreSQL" />
      </NFormItem>
      <NFormItem label="驱动" path="driver" required>
        <NSelect
          v-model:value="formModel.driver"
          :options="driverOptions"
          placeholder="选择数据库类型"
        />
      </NFormItem>
      <NFormItem label="主机" path="host" required>
        <NInput v-model:value="formModel.host" placeholder="localhost" />
      </NFormItem>
      <NFormItem label="端口" path="port">
        <NInputNumber
          v-model:value="formModel.port"
          :min="1"
          :max="65535"
          style="width: 100%"
        />
      </NFormItem>
      <NFormItem label="数据库" path="database">
        <NInput v-model:value="formModel.database" placeholder="mydb" />
      </NFormItem>
      <NFormItem label="用户名" path="username">
        <NInput v-model:value="formModel.username" placeholder="postgres" />
      </NFormItem>
      <NFormItem label="密码" path="password">
        <NInput
          v-model:value="formModel.password"
          type="password"
          show-password-on="click"
          placeholder="••••••••"
        />
      </NFormItem>
      <NFormItem label="SSL">
        <NSwitch v-model:value="formModel.ssl" />
      </NFormItem>
    </NForm>
    <template #footer>
      <NButton @click="testConnection" :loading="testing">测试连接</NButton>
      <NButton type="primary" @click="save" :loading="loading">保存</NButton>
    </template>
  </NModal>
</template>
