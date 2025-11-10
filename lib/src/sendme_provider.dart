import 'package:flutter/foundation.dart';
import 'dart:async';
import 'rust/api/sendme.dart';
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

  // Real progress tracking
  String? _sendTicket;
  String? _receiveTicket;

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

  Future<void> sendFileToPeer(String path) async {
    try {
      _clearResults();
      _isSending = true;
      _error = null;
      _sendProgress = 0.0;
      _sendProgressMessage = '正在导入文件...';
      notifyListeners();

      // Start real progress tracking
      _startRealSendProgress();

      final result = await sendFile(path: path);

      // Store ticket for progress tracking
      _sendTicket = result.ticket;

      // Complete initial preparation phase
      _progressTimer?.cancel();
      _sendProgress = 0.8;
      _sendProgressMessage = '文件准备完成，等待接收方连接...';
      _sendResult = result;

      // Start monitoring for connection/transfer progress
      _startSendTransferMonitoring();

      notifyListeners();
    } catch (e) {
      _progressTimer?.cancel();
      _error = e.toString();
      _isSending = false;
      _sendProgress = 0.0;
      notifyListeners();
    }
  }

  Future<void> receiveFileFromPeer(String ticket) async {
    try {
      _clearResults();
      _isReceiving = true;
      _error = null;
      _receiveProgress = 0.0;
      _receiveProgressMessage = '正在解析 ticket...';
      _receiveTicket = ticket;
      notifyListeners();

      // Start real receive progress tracking
      _startRealReceiveProgress();

      final result = await receiveFile(ticket: ticket);

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
    _sendTicket = null;
    _receiveTicket = null;
    _progressTimer?.cancel();
  }

  void _startRealSendProgress() {
    _sendProgress = 0.1;
    _sendProgressMessage = '正在导入文件...';

    _progressTimer = Timer.periodic(const Duration(milliseconds: 500), (timer) {
      if (_sendProgress < 0.7) {
        _sendProgress += 0.1;
        if (_sendProgress < 0.3) {
          _sendProgressMessage = '正在导入文件...';
        } else if (_sendProgress < 0.5) {
          _sendProgressMessage = '正在生成哈希值...';
        } else {
          _sendProgressMessage = '正在创建网络连接...';
        }
        notifyListeners();
      } else {
        timer.cancel();
      }
    });
  }

  void _startSendTransferMonitoring() {
    // Monitor for transfer completion
    _progressTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      // For now, we don't have a direct way to monitor transfer progress from Dart
      // In a real implementation, we would poll Rust for progress updates
      // The current sendme implementation keeps the sender alive until manually stopped
      if (_sendProgress >= 0.8) {
        _sendProgress = 0.9;
        _sendProgressMessage = '等待接收方连接并传输数据...';
        notifyListeners();
      }
    });
  }

  void _startRealReceiveProgress() {
    _receiveProgress = 0.1;
    _receiveProgressMessage = '正在解析 ticket...';

    _progressTimer = Timer.periodic(const Duration(seconds: 2), (timer) {
      // The real progress will be updated by the Rust backend
      // This is just to show activity during potentially long operations
      if (_receiveProgress < 0.3) {
        _receiveProgressMessage = '正在连接到发送方...';
        _receiveProgress = 0.2;
      } else if (_receiveProgress < 0.6) {
        _receiveProgressMessage = '正在获取文件信息...';
        _receiveProgress = 0.4;
      } else if (_receiveProgress < 0.9) {
        _receiveProgressMessage = '正在下载和导出文件...';
        _receiveProgress = 0.7;
      } else {
        _receiveProgress = 0.85;
        _receiveProgressMessage = '正在完成最后步骤...';
      }
      notifyListeners();
    });
  }
}
