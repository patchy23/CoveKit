/**
 * 工具目录自注册引导
 * main.ts 副作用导入本文件即完成全部工具注册；
 * 新增工具 = 建目录 + 注册文件 + 此处加一行（框架零改动）。
 */
import "@/tools/json-formatter";
import "@/tools/xml-formatter";
import "@/tools/ts-converter";
import "@/tools/base64";
import "@/tools/url-codec";
import "@/tools/random-password";
import "@/tools/color-picker";
import "@/tools/qrcode";
import "@/tools/http-ws";
import "@/tools/sqlite";
import "@/tools/hosts";
