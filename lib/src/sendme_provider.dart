import 'package:flutter/foundation.dart';
import 'dart:async';
import 'rust/frb_generated.dart';
import 'rust/lib.dart';

class SendmeProvider extends ChangeNotifier {
  bool _isSending = false;
  bool _isReceiving = false;
  String? _error;
  SendResult? _sendResult;
  ReceiveResult? _receiveResult;

  // Progress tracking
  double _sendProgress = 0.0;
  double _receiveProgress = 0.0;
  String _sendProgressMessage = '准备发送...';
  String _receiveProgressMessage = '准备接收...';
  Timer? _progressTimer;

  // Getters
  bool get isSending => _isSending;
  bool get isReceiving => _isReceiving;
  String? get error => _error;
  SendResult? get sendResult => _sendResult;
  ReceiveResult? get receiveResult => _receiveResult;
  double get sendProgress => _sendProgress;
  double get receiveProgress => _receiveProgress;
  String get sendProgressMessage => _sendProgressMessage;
  String get receiveProgressMessage => _receiveProgressMessage;

  Future<void> sendFile(String path) async {
    try {
      _clearResults();
      _isSending = true;
      _error = null;
      _sendProgress = 0.0;
      _sendProgressMessage = '正在导入文件...';
      notifyListeners();

      // Start progress simulation
      _startSendProgressSimulation();

      final result = await RustLib.instance.api.crateApiSendmeSendFile(
        path: path,
      );

      // Complete progress
      _progressTimer?.cancel();
      _sendProgress = 1.0;
      _sendProgressMessage = '准备完成，等待接收方连接传输';
      _sendResult = result;
      _isSending = false;
      notifyListeners();
    } catch (e) {
      _progressTimer?.cancel();
      _error = e.toString();
      _isSending = false;
      _sendProgress = 0.0;
      notifyListeners();
    }
  }

  Future<void> receiveFile(String ticket) async {
    try {
      _clearResults();
      _isReceiving = true;
      _error = null;
      _receiveProgress = 0.0;
      _receiveProgressMessage = '正在连接到发送方...';
      notifyListeners();

      // Start progress simulation
      _startReceiveProgressSimulation();

      final result = await RustLib.instance.api.crateApiSendmeReceiveFile(
        ticket: ticket,
      );

      // Complete progress
      _progressTimer?.cancel();
      _receiveProgress = 1.0;
      _receiveProgressMessage = '接收完成！';
      _receiveResult = result;
      _isReceiving = false;
      notifyListeners();
    } catch (e) {
      _progressTimer?.cancel();
      _error = e.toString();
      _isReceiving = false;
      _receiveProgress = 0.0;
      notifyListeners();
    }
  }

  void clearError() {
    _error = null;
    notifyListeners();
  }

  void _clearResults() {
    _sendResult = null;
    _receiveResult = null;
    _error = null;
    _sendProgress = 0.0;
    _receiveProgress = 0.0;
    _sendProgressMessage = '准备发送...';
    _receiveProgressMessage = '准备接收...';
    _progressTimer?.cancel();
  }

  void _startSendProgressSimulation() {
    _sendProgress = 0.1;
    _sendProgressMessage = '正在处理文件...';

    _progressTimer = Timer.periodic(const Duration(milliseconds: 300), (timer) {
      if (_sendProgress < 0.9) {
        _sendProgress += 0.05;
        if (_sendProgress < 0.2) {
          _sendProgressMessage = '正在导入文件...';
        } else if (_sendProgress < 0.4) {
          _sendProgressMessage = '正在生成哈希值...';
        } else if (_sendProgress < 0.6) {
          _sendProgressMessage = '正在创建网络连接...';
        } else if (_sendProgress < 0.8) {
          _sendProgressMessage = '等待接收方连接...';
        } else {
          _sendProgressMessage = '正在传输数据...';
        }
        notifyListeners();
      } else {
        timer.cancel();
      }
    });
  }

  void _startReceiveProgressSimulation() {
    _receiveProgress = 0.1;
    _receiveProgressMessage = '正在解析 ticket...';

    _progressTimer = Timer.periodic(const Duration(milliseconds: 300), (timer) {
      if (_receiveProgress < 0.9) {
        _receiveProgress += 0.05;
        if (_receiveProgress < 0.2) {
          _receiveProgressMessage = '正在解析 ticket...';
        } else if (_receiveProgress < 0.3) {
          _receiveProgressMessage = '正在连接到发送方...';
        } else if (_receiveProgress < 0.7) {
          _receiveProgressMessage = '正在下载文件数据...';
        } else {
          _receiveProgressMessage = '正在导出文件...';
        }
        notifyListeners();
      } else {
        timer.cancel();
      }
    });
  }
}
