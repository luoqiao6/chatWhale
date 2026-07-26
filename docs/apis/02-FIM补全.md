# FIM 补全（Beta）

POST https://api.deepseek.com/beta/completions

FIM（Fill-In-the-Middle）补全 API。

## Request​

### Body
  - **stop**
    一个 string 或最多包含 16 个 string 的 list，在遇到这些词时，API 将停止生成更多的 token。
  - **stream_options** `object` *nullable*
    - **include_usage** `boolean`
      如果设置为 true，在流式消息最后的 data: [DONE] 之前将会传输一个额外的块。此块上的 usage 字段显示整个请求的 token 使用统计信息，而 choices 字段将始终是一个空数组。所有其他块也将包含一个 usage 字段，但其值为 null。
    流式输出相关选项。只有在 stream 参数为 true 时，才可设置此参数。
  - **model** `string` *required*
    Possible values: [deepseek-v4-pro]
  - **prompt** `string` *required*
    Default value: Once upon a time,
  - **echo** `boolean` *nullable*
    在输出中，把 prompt 的内容也输出出来
  - **logprobs** `integer` *nullable*
    Possible values:
  - **max_tokens** `integer` *nullable*
    最大生成 token 数量。
  - **stream** `boolean` *nullable*
    如果设置为 True，将会以 SSE（server-sent events）的形式以流式发送消息增量。消息流以 data: [DONE] 结尾。
  - **suffix** `string` *nullable*
    制定被补全内容的后缀。
  - **temperature** `number` *nullable*
    Possible values:
  - **top_p** `number` *nullable*
    Possible values:
  - **frequency_penalty**
    该参数已不再支持。传入该参数将不会产生任何效果。
  - **presence_penalty**
    该参数已不再支持。传入该参数将不会产生任何效果。
  模型的 ID
  用于生成完成内容的提示
  制定输出中包含 logprobs 最可能输出 token 的对数概率，包含采样的 token。例如，如果 logprobs 是 20，API 将返回一个包含 20 个最可能的 token 的列表。API 将始终返回采样 token 的对数概率，因此响应中可能会有最多 logprobs+1 个元素。
  logprobs 的最大值是 20。
  Default value: 1
  采样温度，介于 0 和 2 之间。更高的值，如 0.8，会使输出更随机，而更低的值，如 0.2，会使其更加集中和确定。 我们通常建议可以更改这个值或者更改 top_p，但不建议同时对两者进行修改。
  Default value: 1
  作为调节采样温度的替代方案，模型会考虑前 top_p 概率的 token 的结果。所以 0.1 就意味着只有包括在最高 10% 概率中的 token 会被考虑。 我们通常建议修改这个值或者更改 temperature，但不建议同时对两者进行修改。

## Responses​

OK

- **Schema**
  - **choices** `object[]` *required*
    - **logprobs** `object` *required* *nullable*
      - **text_offset** `integer[]`
      - **token_logprobs** `number[]`
      - **tokens** `string[]`
      - **top_logprobs** `object[]`
    - **finish_reason** `string` *required*
      Possible values: [stop, length, content_filter, insufficient_system_resource]
    - **index** `integer` *required*
    - **text** `string` *required*
    模型生成的补全内容的选择列表。
    模型停止生成 token 的原因。
    stop：模型自然停止生成，或遇到 stop 序列中列出的字符串。
    length ：输出长度达到了模型上下文长度限制，或达到了 max_tokens 的限制。
    content_filter：输出内容因触发过滤策略而被过滤。
    insufficient_system_resource: 由于后端推理资源受限，请求被打断。
  - **usage** `object`
    - **completion_tokens_details** `object`
      - **reasoning_tokens** `integer`
        推理模型所产生的思维链 token 数量
      completion tokens 的详细信息。
    - **completion_tokens** `integer` *required*
      模型 completion 产生的 token 数。
    - **prompt_tokens** `integer` *required*
      用户 prompt 所包含的 token 数。该值等于 prompt_cache_hit_tokens + prompt_cache_miss_tokens
    - **prompt_cache_hit_tokens** `integer` *required*
      用户 prompt 中，命中上下文缓存的 token 数。
    - **prompt_cache_miss_tokens** `integer` *required*
      用户 prompt 中，未命中上下文缓存的 token 数。
    - **total_tokens** `integer` *required*
      该请求中，所有 token 的数量（prompt + completion）。
    该对话补全请求的用量信息。
  - **id** `string` *required*
    补全响应的 ID。
  - **created** `integer` *required*
    标志补全请求开始时间的 Unix 时间戳（以秒为单位）。
  - **model** `string` *required*
    补全请求所用的模型。
  - **system_fingerprint** `string`
    模型运行时的后端配置的指纹。
  - **object** `string` *required*
    Possible values: [text_completion]
  object 的类型，一定为"text_completion"

```json
{
  "id": "string",
  "choices": [
    {
      "finish_reason": "stop",
      "index": 0,
      "logprobs": {
        "text_offset": [
          0
        ],
        "token_logprobs": [
          0
        ],
        "tokens": [
          "string"
        ],
        "top_logprobs": [
          {}
        ]
      },
      "text": "string"
    }
  ],
  "created": 0,
  "model": "string",
  "system_fingerprint": "string",
  "object": "text_completion",
  "usage": {
    "completion_tokens": 0,
    "prompt_tokens": 0,
    "prompt_cache_hit_tokens": 0,
    "prompt_cache_miss_tokens": 0,
    "total_tokens": 0,
    "completion_tokens_details": {
      "reasoning_tokens": 0
    }
  }
}
```